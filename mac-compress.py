#!/usr/bin/env python3.9
import argparse
import os
import re
import sys
import datetime
import asyncio
from limiter import get_limiter, limit
from functions import cprint, bytes2human, filter_filenames, smartProcessFile


async def main():
    startTime = datetime.datetime.now()
    parser = argparse.ArgumentParser(description="compress images")
    parser.add_argument("-v", action="store_true", help="enables verbose text output")
    parser.add_argument("-q", action="store_true", help="quiet, silences all text output")
    parser.add_argument("--lossy", action="store_true", help="default false, enables lossy optimzations")
    parser.add_argument(
        "--zopfli",
        action="store_true",
        help="default false, enables zopfli compression for PNGs (slow & CPU intenstive)",
    )
    parser.add_argument(
        "--parallelism", metavar="p", type=int, default=4, help="how many async threads to allow, default=4"
    )
    parser.add_argument("files", metavar="F", type=str, nargs="+", help="any files or folders to process")
    args = parser.parse_args()
    assert not (args.v and args.q), "ERROR: can't have verbose (-v) and quiet (-q) flags enabled at once"

    # We want to rate limit the number of processes here via cli, apply decorator to create new function
    # https://github.com/alexdelorenzo/limiter
    limit_operations = limit(get_limiter(rate=args.parallelism, capacity=args.parallelism), consume=1)
    limitedSmartProcessFile = limit_operations(smartProcessFile)

    if args.v:
        if args.lossy:
            cprint(f"Lossy processing enabled: {sys.argv}", color="yellow")
        else:
            cprint(f"Lossy processing off: {sys.argv}", color="green")
        if args.zopfli:
            cprint("Warning: zopfli processing enabled. This can be slow!", color="yellow")

    # match for all image formats we work with, then walk dirs to match regex
    reg_expr = re.compile(r".*\.(jpg|png|jpeg|gif)$", re.IGNORECASE).match
    imageFiles = []
    sourceSizes = 0
    for inputFile in args.files:
        if os.path.isfile(inputFile):
            if reg_expr(inputFile):
                originalSize = os.path.getsize(inputFile)
                sourceSizes += originalSize
                imageFiles.append((inputFile, originalSize))
            else:
                cprint(f"no processing, unknown file type for {inputFile}", "red")
        else:  # it's a dir, find images beaneath
            for filename in filter_filenames(inputFile, reg_expr):
                originalSize = os.path.getsize(filename)
                sourceSizes += originalSize
                imageFiles.append((filename, originalSize))

    resultSizes = await asyncio.gather(
        *(limitedSmartProcessFile(inputTuple, args.v, args.q, args.lossy, args.zopfli) for inputTuple in imageFiles)
    )
    if not args.q:
        cprint(
            f"---Totals---\nOriginal size {bytes2human(sourceSizes)} | "
            + f"Final size {bytes2human(sum(resultSizes))} | Saved {bytes2human(sourceSizes - sum(resultSizes))}",
        )
    if args.v:
        print(f"Processing time {datetime.datetime.now()-startTime}")


if __name__ == "__main__":
    asyncio.run(main())

#!/usr/bin/env python3
import argparse
import os
import re
from sys import platform, argv
import datetime
import asyncio
from sys import exit
from limiter import get_limiter, limit
from functions import CompressHistory, cprint, bytes2human, filter_filenames, smartProcessFile


async def main():
    startTime = datetime.datetime.now()
    parser = argparse.ArgumentParser(description="compress images")
    parser.add_argument("-v", action="store_true", help="default false, enables verbose text output")
    parser.add_argument("-q", action="store_true", help="default false, quiet, silences all text output")
    parser.add_argument("--lossy", action="store_true", help="default false, enables lossy optimzations")
    parser.add_argument("--jpg", action="store_true", help="default false, convert images to JPEGs")
    parser.add_argument("--png", action="store_true", help="default false, convert images to PNGs")
    parser.add_argument(
        "--deleteOnConvert",
        action="store_true",
        help="default false, when converting PNG to JPG or vice versa delete orig",
    )
    parser.add_argument(
        "--zopfli",
        action="store_true",
        help="default false, enables zopfli compression for PNGs (slow & CPU intenstive)",
    )
    parser.add_argument(
        "--parallelism", metavar="p", type=int, default=4, help="how many async threads to allow, default=4"
    )
    parser.add_argument(
        "--history",
        metavar="d",
        type=str,
        default=None,
        help="a file that tracks images that have already been compressed",
    )
    parser.add_argument("files", metavar="F", type=str, nargs="+", help="any files or folders to process")
    args = parser.parse_args()
    assert not (args.v and args.q), "ERROR: can't have verbose (-v) and quiet (-q) flags enabled at once"
    assert not (args.jpg and args.png), "ERROR: can't have png (--png) and jpg (--jpg) flags enabled at once"

    # We want to rate limit the number of processes here via cli, apply decorator to create new function
    # https://github.com/alexdelorenzo/limiter
    limit_operations = limit(get_limiter(rate=args.parallelism, capacity=args.parallelism), consume=1)
    limitedSmartProcessFile = limit_operations(smartProcessFile)

    if args.v:
        if args.lossy:
            cprint(f"Lossy processing enabled: {argv}", color="yellow")
        else:
            cprint(f"Lossy processing off: {argv}", color="green")
        if args.zopfli:
            cprint("Warning: zopfli processing enabled. This can be slow!", color="yellow")
    if not args.q and (args.jpg or args.png):
        if args.jpg:
            message = "Warning: will convert all images to JPEGs."
        elif args.png:
            message = "Warning: will convert all images to PNGs."
        if args.deleteOnConvert:
            message += " Original images will be deleted after format conversion!"
        cprint(message, color="yellow")

    # we'll use this to help find the right paths later
    if platform.startswith("linux"):
        args.platform = "linux"
    elif platform == "darwin":
        args.platform = "mac"
    elif platform == "win32":
        args.platform = "win"

    # set up our SQLite history object
    historyObj = CompressHistory(args.history) if args.history else None

    # match for all image formats we work with, then walk dirs to match regex
    reg_expr = re.compile(r".*\.(jpg|png|jpeg|gif)$", re.IGNORECASE).match
    imageFiles = []
    sourceSizes = 0
    for inputFile in args.files:
        if os.path.isfile(inputFile):
            if reg_expr(inputFile):
                originalSize = os.path.getsize(inputFile)
                sourceSizes += originalSize
                if historyObj:
                    imageFiles = historyObj.appendToListIfNotInArchive(imageFiles, inputFile, originalSize, args)
                else:
                    imageFiles.append((inputFile, originalSize))
            else:
                cprint(f"no processing, unknown file type for {inputFile}", "red")
        else:  # it's a dir, find images beneath
            for filename in filter_filenames(inputFile, reg_expr):
                originalSize = os.path.getsize(filename)
                sourceSizes += originalSize
                if historyObj:
                    imageFiles = historyObj.appendToListIfNotInArchive(imageFiles, filename, originalSize, args)
                else:
                    imageFiles.append((filename, originalSize))

    results = await asyncio.gather(
        *(
            limitedSmartProcessFile(
                inputTuple[0],
                inputTuple[1],
                args,
                historyObj,
            )
            for inputTuple in imageFiles
        )
    )
    if len(results) > 0:
        if not args.q:
            totalResultSize = sum(i for i, _ in results)
            # TODO this math might change for previously processed files we skip
            cprint(
                f"---Totals---\nOriginal size {bytes2human(sourceSizes)} | Final size {bytes2human(totalResultSize)} | "
                + f"Saved {bytes2human(sourceSizes - totalResultSize)} ({1- totalResultSize/sourceSizes:.2%})",
            )
    else:
        cprint("No files processed", "red")
        exit(1)
    if args.v:
        print(f"Processing time {datetime.datetime.now()-startTime}")


if __name__ == "__main__":
    asyncio.run(main())

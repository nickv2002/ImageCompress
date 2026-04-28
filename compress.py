#!/usr/bin/env python3
# Run with: uv run compress.py [args]
import argparse
import asyncio
import datetime
import os
import re
from sys import argv, exit

from limiter import Limiter

from functions import (
    CompressHistory,
    bytes2human,
    cprint,
    filter_filenames,
    smartProcessFile,
)


async def main():
    startTime = datetime.datetime.now()
    parser = argparse.ArgumentParser(description="compress images")
    parser.add_argument(
        "-v", action="store_true", help="default false, enables verbose text output"
    )
    parser.add_argument(
        "-q", action="store_true", help="default false, quiet, silences all text output"
    )
    parser.add_argument(
        "--lossy", action="store_true", help="default false, enables lossy optimzations"
    )
    parser.add_argument(
        "--jpg", action="store_true", help="default false, convert images to JPEGs"
    )
    parser.add_argument(
        "--png", action="store_true", help="default false, convert images to PNGs"
    )
    parser.add_argument(
        "--heif", action="store_true", help="default false, convert images to HEIFs"
    )
    parser.add_argument(
        "--avif", action="store_true", help="default false, convert images to AVIFs"
    )
    parser.add_argument(
        "--jxl", action="store_true", help="default false, convert images to JPEG-XL"
    )
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
        "--parallelism",
        metavar="p",
        type=int,
        default=4,
        help="how many async threads to allow, default=4",
    )
    parser.add_argument(
        "--history",
        metavar="d",
        type=str,
        default=None,
        help="a file that tracks images that have already been compressed",
    )
    parser.add_argument(
        "files",
        metavar="F",
        type=str,
        nargs="+",
        help="any files or folders to process",
    )
    args = parser.parse_args()
    if args.v and args.q:
        parser.error("can't have verbose (-v) and quiet (-q) flags enabled at once")
    if sum([args.jpg, args.png, args.heif, args.avif, args.jxl]) > 1:
        parser.error("can't have multiple output format flags (--png, --jpg, --heif, --avif)")

    # We want to rate limit the number of processes here via cli, apply decorator to create new function
    limiter = Limiter(rate=args.parallelism)
    limitedSmartProcessFile = limiter(smartProcessFile)

    if args.v:
        if args.lossy:
            cprint(f"Lossy processing enabled: {argv}", color="yellow")
        else:
            cprint(f"Lossy processing off: {argv}", color="green")
        if args.zopfli:
            cprint(
                "Warning: zopfli processing enabled. This can be slow!", color="yellow"
            )
    if not args.q and (args.jpg or args.png):
        message = ""
        if args.jpg:
            message = "Warning: will convert all images to JPEGs."
        elif args.png:
            message = "Warning: will convert all images to PNGs."
        elif args.heif:
            message = "Warning: will convert all images to HEIFs."
        elif args.jxl:
            message = "Warning: will convert all images to JPEG-XL."
        if args.deleteOnConvert:
            message += " Original images will be deleted after format conversion!"
        cprint(message, color="yellow")

    # set up our SQLite history object
    historyObj = CompressHistory(args.history) if args.history else None

    # match for all image formats we work with, then walk dirs to match regex
    reg_expr = re.compile(
        r".*\.(jpg|png|jpeg)$", re.IGNORECASE
    ).match
    imageFiles = []
    sourceSizes = 0
    for inputFile in args.files:
        if os.path.isfile(inputFile):
            if reg_expr(inputFile):
                originalSize = os.path.getsize(inputFile)
                sourceSizes += originalSize
                if historyObj:
                    imageFiles = historyObj.appendToListIfNotInArchive(
                        imageFiles, inputFile, originalSize, args
                    )
                else:
                    imageFiles.append((inputFile, originalSize))
            else:
                cprint(f"no processing, unknown file type for {inputFile}", "red")
        else:  # it's a dir, find images beneath
            for filename in filter_filenames(inputFile, reg_expr):
                originalSize = os.path.getsize(filename)
                sourceSizes += originalSize
                if historyObj:
                    imageFiles = historyObj.appendToListIfNotInArchive(
                        imageFiles, filename, originalSize, args
                    )
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

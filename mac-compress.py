#!/usr/bin/env python3.9
import argparse
import os
import re
import shutil
import sys
import datetime
from click import echo, style
import asyncio
from limiter import get_limiter, limit


def cprint(text: str, color: str = "white"):
    echo(style(text, fg=color))


def compareAndKeepSmaller(oldFile: str, newFile: str):
    oldFileSize = os.path.getsize(oldFile)
    newFileSize = os.path.getsize(newFile)
    if newFileSize == oldFileSize:
        if verbose:
            cprint(f"new file is same size, no update to {oldFile}", "blue")
        os.remove(newFile)
    elif newFileSize < oldFileSize:
        if verbose:
            cprint(f"new file is {1 - newFileSize / oldFileSize:.2%} smaller! Updating it at {oldFile}", "green")
        shutil.move(newFile, oldFile)
    else:
        if verbose:
            cprint(f"old file is smaller, keeping {oldFile}", "magenta")
        os.remove(newFile)


# regex filename filter
def filter_filenames(dirpath, predicate):
    for dir_, dirnames, filenames in os.walk(dirpath):
        for filename in filenames:
            abspath = os.path.join(dir_, filename)
            if predicate(abspath):
                yield abspath


async def subProcessToStr(args: list, stdErrIsErr: bool = True) -> str:
    calledCmd = " ".join(args)
    if verbose:
        print(calledCmd)
    proc = await asyncio.create_subprocess_exec(
        args[0], *args[1:], stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE
    )
    stdout, stderr = await proc.communicate()
    if stderr and stdErrIsErr:
        cprint(f"*stderr for {calledCmd}]\n{stderr.decode('utf-8')}*", "yellow")
        raise TypeError
    if not stdErrIsErr and verbose:
        print(stderr.decode("utf-8"))
    return stdout.decode("utf-8")


# pngquant is a util for lossy compression of PNG images.
# very lossy - uses a reduced number of colors
async def pngquant(inFile: str) -> str:
    args = [
        "/opt/homebrew/bin/pngquant",
        "--force",  # allow overwrite so we can update-in-place
        "--skip-if-larger",  # only save converted files if they're smaller than original
        "--strip",  # remove optional metadata (default on Mac)
        "--quality=85-90",  # don't save below min, use fewer colors below max (0-100)
        "--output",
        inFile,  # will output to input
        "--",  # Stops processing of arguments
        inFile,
    ]
    return await subProcessToStr(args)


# Oxipng is a multithreaded lossless PNG compression optimizer
async def oxipng(inFile: str) -> str:
    args = [
        "/opt/homebrew/bin/oxipng",
        "--preserve",  # Preserve file attributes if possible
        "-s",  # Strip safely-removable metadata objects
        "--alpha",  # Perform additional alpha optimizations
        "--interlace",
        "0",  # disable interlacing
        inFile,  # will output to input
    ]
    if doZopfli:
        args.append("--zopfli")  # Use the slower but better compressing Zopfli algorithm
    return await subProcessToStr(args, stdErrIsErr=False)


# MozJPG is lossless JPG comression
async def mozjpg(inFile: str):
    outfile = os.path.join("/tmp", "mozjpg-" + os.path.split(inFile)[1])
    args = [
        "/opt/homebrew/Cellar/mozjpeg/4.0.3/bin/jpegtran",
        "-optimize",  # Optimize Huffman table (smaller file, but slow compression)
        "-progressive",  # Create progressive JPEG file
        "-copy",  # Strip safely-removable metadata objects
        "none",  # Copy no extra markers from source file
        "-outfile",
        outfile,  # doesn't check if result it smaller
        inFile,
    ]
    returnedText = await subProcessToStr(args, stdErrIsErr=False)
    compareAndKeepSmaller(inFile, outfile)
    return returnedText


# cjpeg is lossy JPG compression
# some mozjpg defaults https://calendar.perfplanet.com/2014/mozjpeg-3-0/
# ./cjpeg -quality 70 -outfile compressed-image.jpg source-image.png
async def cjpeg(inFile: str):
    outfile = os.path.join("/tmp", "cjpeg-" + os.path.split(inFile)[1])
    args = [
        "/opt/homebrew/Cellar/mozjpeg/4.0.3/bin/cjpeg",
        "-quality",  # Compression quality (0..100; 5-95 is most useful range, default is 75)
        "85",  # TODO make this more adjustable?
        "-sample",
        "1x1",  # enables full-resolution color (e.g. red lines won’t be smudged as badly), but it makes files larger.
        "-outfile",
        outfile,  # will output to input, doesn't check if result it smaller
        inFile,  # note the input could be non-jpeg
    ]
    returnedText = await subProcessToStr(args, stdErrIsErr=False)
    compareAndKeepSmaller(inFile, outfile)
    return returnedText


# gifsicle can do lossy and lossless compression
# https://www.lcdf.org/gifsicle/man.html
# lossless: gifsicle -O3 test-7.gif  -o test-7-update.gif
# lossy: : gifsicle -O3 --lossy test-7.gif  -o test-7-update.gif
async def gifsicle(inFile: str):
    outfile = os.path.join("/tmp", "gifsicle-" + os.path.split(inFile)[1])
    args = [
        "/opt/homebrew/bin/gifsicle",
        "-O3",  # Try several optimization methods (usually slower, sometimes better results)
        inFile,
        "-o",
        outfile,  # will output to input, doesn't check if result it smaller
    ]
    if doLossy:
        args.append("--lossy")  # TODO make this more adjustable? default number is 20
    returnedText = await subProcessToStr(args, stdErrIsErr=False)
    compareAndKeepSmaller(inFile, outfile)
    return returnedText


async def smartProcessFile(inputFile: str):
    assert os.path.isfile(inputFile), f"ERROR: missing file {inputFile}"
    assert os.access(inputFile, os.W_OK), f"ERROR: file {inputFile} is not writeable"
    # TODO: get file size before and after, print if wanted
    fileExt = os.path.splitext(inputFile)[1].lower()
    if fileExt == ".png":
        if doLossy:
            result = await pngquant(inputFile)
            if verbose:
                print(result)
        result = await oxipng(inputFile)
        if verbose:
            print(result)
    elif fileExt == ".gif":
        result = await gifsicle(inputFile)
        if verbose:
            print(result)
    elif fileExt == ".jpg" or fileExt == ".jpeg":
        if doLossy:
            result = await cjpeg(inputFile)
            if verbose:
                print(result)
        result = await mozjpg(inputFile)
        if verbose:
            print(result)
    else:
        assert False, f"error, this should not be here, no processing, unknown filetime for {inputFile}"


async def main():
    startTime = datetime.datetime.now()
    parser = argparse.ArgumentParser(description="compress images")
    parser.add_argument("-v", action="store_true", help="enables verbose text output")
    parser.add_argument("--lossy", action="store_true", help="default false, enables lossy optimzations")
    parser.add_argument(
        "--zopfli",
        action="store_true",
        help="default false, enables zopfli compression for PNGs (slow & CPU inensive)",
    )
    parser.add_argument(
        "--parallelism", metavar="p", type=int, default=4, help="how many async threads to allow, default=4"
    )
    parser.add_argument("files", metavar="F", type=str, nargs="+", help="any files or folders to process")
    args = parser.parse_args()
    global doLossy, doZopfli, verbose
    doLossy = args.lossy
    doZopfli = args.zopfli
    verbose = args.v
    # We want to rate limit the number of processes here via cli, appy decorator to create new function
    # https://github.com/alexdelorenzo/limiter
    limit_operations = limit(get_limiter(rate=args.parallelism, capacity=args.parallelism), consume=1)
    limitedSmartProcessFile = limit_operations(smartProcessFile)

    if verbose:
        if doLossy:
            cprint(f"Lossy processing enabled: {sys.argv}", color="yellow")
        else:
            cprint(f"Lossy processing off: {sys.argv}", color="green")
        if doZopfli:
            cprint("Zopfli processing enabled, can be slow!", color="yellow")

    # match for all image formats we work with, then walk dirs to match regex
    reg_expr = re.compile(r".*\.(jpg|png|jpeg|gif)$", re.IGNORECASE).match
    imageFiles = []
    for inputFile in args.files:
        if os.path.isfile(inputFile):
            if reg_expr(inputFile):
                imageFiles.append(inputFile)
            else:
                cprint(f"no processing, unknown file type for {inputFile}", "red")
        else:  # it's a dir, find images beaneath
            for filename in filter_filenames(inputFile, reg_expr):
                imageFiles.append(filename)

    await asyncio.gather(*(limitedSmartProcessFile(inputFile) for inputFile in imageFiles))

    if verbose:
        print(f"Processing time {datetime.datetime.now()-startTime}")


if __name__ == "__main__":
    asyncio.run(main())

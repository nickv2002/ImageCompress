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


SYMBOLS = {
    "customary": ("B", "K", "M", "G", "T", "P", "E", "Z", "Y"),
    "customary_ext": ("byte", "kilo", "mega", "giga", "tera", "peta", "exa", "zetta", "iotta"),
    "iec": ("Bi", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi"),
    "iec_ext": ("byte", "kibi", "mebi", "gibi", "tebi", "pebi", "exbi", "zebi", "yobi"),
}


def bytes2human(n: int, format: str = "%(value).1f %(symbol)s", symbols: str = "customary") -> str:
    """
    Convert n bytes into a human readable string based on format.
    symbols can be either "customary", "customary_ext", "iec" or "iec_ext",
    see: http://goo.gl/kTQMs

      >>> bytes2human(0)
      '0.0 B'
      >>> bytes2human(0.9)
      '0.0 B'
      >>> bytes2human(1)
      '1.0 B'
      >>> bytes2human(1.9)
      '1.0 B'
      >>> bytes2human(1024)
      '1.0 K'
      >>> bytes2human(1048576)
      '1.0 M'
      >>> bytes2human(1099511627776127398123789121)
      '909.5 Y'

      >>> bytes2human(9856, symbols="customary")
      '9.6 K'
      >>> bytes2human(9856, symbols="customary_ext")
      '9.6 kilo'
      >>> bytes2human(9856, symbols="iec")
      '9.6 Ki'
      >>> bytes2human(9856, symbols="iec_ext")
      '9.6 kibi'

      >>> bytes2human(10000, "%(value).1f %(symbol)s/sec")
      '9.8 K/sec'

      >>> # precision can be adjusted by playing with %f operator
      >>> bytes2human(10000, format="%(value).5f %(symbol)s")
      '9.76562 K'
    """
    n = int(n)
    if n < 0:
        raise ValueError("n < 0")
    symbols = SYMBOLS[symbols]
    prefix = {}
    for i, s in enumerate(symbols[1:]):
        prefix[s] = 1 << (i + 1) * 10
    for symbol in reversed(symbols[1:]):
        if n >= prefix[symbol]:
            value = float(n) / prefix[symbol]
            return format % locals()
    return format % dict(symbol=symbols[0], value=n)


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


async def smartProcessFile(inputTuple: tuple):  # using a tupal here since it's easier to wrap in gather()
    inputFile, inputSize = inputTuple
    assert os.path.isfile(inputFile), f"ERROR: missing file {inputFile}"
    assert os.access(inputFile, os.W_OK), f"ERROR: file {inputFile} is not writeable"
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

    newSize = os.path.getsize(inputFile)
    if not quiet:
        if newSize < inputSize:
            cprint(
                f"Compressed {inputFile} by {1 - newSize / inputSize:.2%} & saved: {bytes2human(inputSize-newSize)}",
                "green",
            )
        else:
            cprint(f"No space saved for {inputFile}, keeping original.", "red")

    return newSize


async def main():
    startTime = datetime.datetime.now()
    parser = argparse.ArgumentParser(description="compress images")
    parser.add_argument("-v", action="store_true", help="enables verbose text output")
    parser.add_argument("-q", action="store_true", help="quiet, silences all text output")
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
    global doLossy, doZopfli, verbose, quiet
    doLossy = args.lossy
    doZopfli = args.zopfli
    verbose = args.v
    quiet = args.q
    assert not (verbose and quiet), "ERROR: can't have verbose (-v) and quiet (-q) flags enabled at once"
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
            cprint("Warning: qopfli processing enabled. This can be slow!", color="yellow")

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

    resultSizes = await asyncio.gather(*(limitedSmartProcessFile(inputTuple) for inputTuple in imageFiles))
    if not quiet:
        cprint(
            f"---Totals---\nOriginal size {bytes2human(sourceSizes)} | "
            + f"Final size {bytes2human(sum(resultSizes))} | Saved {bytes2human(sourceSizes - sum(resultSizes))}",
        )
    if verbose:
        print(f"Processing time {datetime.datetime.now()-startTime}")


if __name__ == "__main__":
    asyncio.run(main())

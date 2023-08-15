import os
import shutil
import sqlite3
import subprocess
from click import echo, style
import asyncio
from argparse import Namespace as apns  # just importing this for type hinting

SYMBOLS = {
    "customary": ("B", "K", "M", "G", "T", "P", "E", "Z", "Y"),
    "customary_ext": ("byte", "kilo", "mega", "giga", "tera", "peta", "exa", "zetta", "iotta"),
    "iec": ("Bi", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi"),
    "iec_ext": ("byte", "kibi", "mebi", "gibi", "tebi", "pebi", "exbi", "zebi", "yobi"),
}

# TODO use shutl.which() to look up these paths
PATHS = {
    "mac": {
        "pngquant": "/opt/homebrew/bin/pngquant",
        "oxipng": "/opt/homebrew/bin/oxipng",
        "jpegtran": "/opt/homebrew/Cellar/mozjpeg/4.1.1/bin/jpegtran",
        "cjpeg": "/opt/homebrew/Cellar/mozjpeg/4.1.1/bin/cjpeg",
        "gifsicle": "/opt/homebrew/bin/gifsicle",
        "convert": "/opt/homebrew/bin/convert",
        "heif-enc": "/opt/homebrew/bin/heif-enc",
    },
    "linux": {
        "pngquant": "/usr/bin/pngquant",
        "oxipng": "/usr/local/bin/oxipng",
        "jpegtran": "/usr/local/bin/jpegtran-static",
        "cjpeg": "/usr/local/bin/cjpeg-static",
        "gifsicle": "/usr/bin/gifsicle",
        "convert": "/usr/bin/convert",
        "heif-enc": "/usr/bin/heif-enc",
    },
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

      Via https://code.activestate.com/recipes/578019-bytes-to-human-human-to-bytes-converter/?in=user-4178764
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


def compareAndKeepSmaller(oldFile: str, newFile: str, args: apns):
    oldFileSize = os.path.getsize(oldFile)
    if not os.path.isfile(newFile):
        cprint(f"WARNING: missing expected output {newFile}", "yellow")
        return
    newFileSize = os.path.getsize(newFile)
    if newFileSize == oldFileSize:
        if args.v:
            cprint(f"new file is same size, no update to {oldFile}", "blue")
        os.remove(newFile)
    elif newFileSize < oldFileSize:
        if args.v:
            cprint(f"new file is {1 - newFileSize / oldFileSize:.2%} smaller! Updating it at {oldFile}", "green")
        shutil.move(newFile, oldFile)
    else:
        if args.v:
            cprint(f"old file is smaller, keeping {oldFile}", "magenta")
        os.remove(newFile)


# regex filename filter
def filter_filenames(dirpath, predicate):
    for dir_, dirnames, filenames in os.walk(dirpath):
        for filename in filenames:
            abspath = os.path.join(dir_, filename)
            if predicate(abspath):
                yield abspath


async def subProcessToStr(subpArgs: list, args: apns, stdErrIsErr: bool = True) -> str:
    calledCmd = " ".join(subpArgs)
    if args.v:
        print(calledCmd)
    proc = await asyncio.create_subprocess_exec(
        subpArgs[0], *subpArgs[1:], stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE
    )
    stdout, stderr = await proc.communicate()
    if stderr and stdErrIsErr:
        cprint(f"*stderr for {calledCmd}]\n{stderr.decode('utf-8')}*", "yellow")
        raise TypeError
    if not stdErrIsErr and args.v:
        print(stderr.decode("utf-8"))
    return stdout.decode("utf-8")


# pngquant is a util for lossy compression of PNG images.
# very lossy - uses a reduced number of colors
async def pngquant(inFile: str, args: apns) -> str:
    subpArgs = [
        PATHS[args.platform]["pngquant"],
        "--force",  # allow overwrite so we can update-in-place
        "--skip-if-larger",  # only save converted files if they're smaller than original
        "--strip",  # remove optional metadata (default on Mac)
        "--quality=85-90",  # don't save below min, use fewer colors below max (0-100)
        "--output",
        inFile,  # will output to input
        "--",  # Stops processing of arguments
        inFile,
    ]
    return await subProcessToStr(subpArgs, args=args)


# Oxipng is a multithreaded lossless PNG compression optimizer
async def oxipng(inFile: str, args: apns) -> str:
    subpArgs = [
        PATHS[args.platform]["oxipng"],
        "--preserve",  # Preserve file attributes if possible
        "-s",  # Strip safely-removable metadata objects
        "--alpha",  # Perform additional alpha optimizations
        "--interlace",
        "0",  # disable interlacing
        inFile,  # will output to input
    ]
    if args.zopfli:
        args.append("--zopfli")  # Use the slower but better compressing Zopfli algorithm
    return await subProcessToStr(subpArgs, stdErrIsErr=False, args=args)


# MozJPG is lossless JPG comression
async def mozjpg(inFile: str, args: apns) -> str:
    outfile = os.path.join("/tmp", "mozjpg-" + os.path.split(inFile)[1])
    subpArgs = [
        PATHS[args.platform]["jpegtran"],
        "-optimize",  # Optimize Huffman table (smaller file, but slow compression)
        "-progressive",  # Create progressive JPEG file
        "-copy",  # Strip safely-removable metadata objects
        "none",  # Copy no extra markers from source file
        "-outfile",
        outfile,  # doesn't check if result it smaller
        inFile,
    ]
    returnedText = await subProcessToStr(subpArgs, stdErrIsErr=False, args=args)
    compareAndKeepSmaller(inFile, outfile, args)
    return returnedText


# cjpeg is lossy JPG compression
# some mozjpg defaults https://calendar.perfplanet.com/2014/mozjpeg-3-0/
# ./cjpeg -quality 70 -outfile compressed-image.jpg source-image.png
# can also convert non-JPEG to JPEG
async def cjpeg(inFile: str, args: apns, quality: int = 85) -> str:
    outfile = os.path.join("/tmp", "cjpeg-" + os.path.split(inFile)[1])
    subpArgs = [
        PATHS[args.platform]["cjpeg"],
        "-quality",  # Compression quality (0..100; 5-95 is most useful range, default is 75)
        f"{quality}",
        "-sample",
        "1x1",  # enables full-resolution color (e.g. red lines won’t be smudged as badly), but it makes files larger.
        "-outfile",
        outfile,  # will output to input, doesn't check if result it smaller
        inFile,  # note the input could be non-jpeg
    ]
    returnedText = await subProcessToStr(subpArgs, stdErrIsErr=False, args=args)
    compareAndKeepSmaller(inFile, outfile, args)
    return returnedText


# gifsicle can do lossy and lossless compression
# https://www.lcdf.org/gifsicle/man.html
# lossless: gifsicle -O3 test-7.gif  -o test-7-update.gif
# lossy: : gifsicle -O3 --lossy test-7.gif  -o test-7-update.gif
async def gifsicle(inFile: str, args: apns) -> str:
    outfile = os.path.join("/tmp", "gifsicle-" + os.path.split(inFile)[1])
    subpArgs = [
        PATHS[args.platform]["gifsicle"],
        "-O3",  # Try several optimization methods (usually slower, sometimes better results)
        inFile,
        "-o",
        outfile,  # will output to input, doesn't check if result it smaller
    ]
    if args.lossy:
        args.append("--lossy")  # TODO make this more adjustable? default number is 20
    returnedText = await subProcessToStr(subpArgs, stdErrIsErr=False, args=args)
    compareAndKeepSmaller(inFile, outfile, args=args)
    return returnedText


# convert with imagemagick
async def convertImageMagick(inFile: str, toFmt: str, args: apns) -> str:
    assert toFmt == "jpg" or toFmt == "png", "ERROR: unsupported conversion format"
    baseFileNameNoExt = os.path.splitext(os.path.split(inFile)[1])[0]
    baseFileExt = os.path.splitext(inFile)[1].lower()
    outFile = os.path.join(os.path.split(inFile)[0], f"{baseFileNameNoExt}.{toFmt}")

    if toFmt == "jpg" and (baseFileExt != ".jpeg" and baseFileExt != ".jpg"):
        pass
    elif toFmt == "png" and (baseFileExt != ".png"):
        pass
    else:
        if args.v:
            cprint(f"{inFile} was already in the desired format ({toFmt}), skipping", "yellow")
        return inFile
    subpArgs = [
        PATHS[args.platform]["convert"],
        inFile,
        outFile,
    ]
    returnText = await subProcessToStr(subpArgs, args=args)
    if args.v:
        print(returnText)
    if args.deleteOnConvert:
        os.remove(inFile)
    if not args.q:
        cprint(f"Converted {inFile} to {outFile}", "blue")
    return outFile


# convert with heif-enc
def convertHEIF(inFile: str, args: apns) -> str:
    baseFileNameNoExt = os.path.splitext(os.path.split(inFile)[1])[0]
    baseFileExt = os.path.splitext(inFile)[1].lower()
    outFile = os.path.join(os.path.split(inFile)[0], f"{baseFileNameNoExt}.heif")
    if baseFileExt == ".gif":
        cprint(f"Warning skipping conversion for {inFile} since we don't want to flatten animated GIFs", "yellow")
        return inFile

    if baseFileExt == ".heif" or baseFileExt == ".heic":
        if args.v:
            cprint(f"{inFile} was already in the desired format ({baseFileExt}), skipping", "yellow")
        return inFile
    # heif-enc $i.jpg --quality $q --output $i-$q.heif
    subpArgs = [
        PATHS[args.platform]["heif-enc"],
        inFile,
        "--quality",
        "50",
        "--output",
        outFile,
    ]
    calledCmd = " ".join(subpArgs)
    if args.v:
        print(calledCmd)
    # proc = await asyncio.create_subprocess_exec(
    #     subpArgs[0], *subpArgs[1:], stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE
    # )
    # stdout, stderr = await proc.communicate()

    p = subprocess.run(subpArgs, capture_output=True)
    stdout = p.stdout
    stderr = p.stderr

    if stderr:
        cprint(f"*stderr for {calledCmd}]\n{stderr.decode('utf-8')}*", "yellow")
        return inFile
    if args.v:
        print(stdout.decode("utf-8"))
    if args.deleteOnConvert:
        os.remove(inFile)
    if not args.q:
        cprint(f"Converted {inFile} to {outFile}", "blue")
    return outFile


class CompressHistory:
    def __init__(self, path):
        try:
            con = sqlite3.connect(path, timeout=60, check_same_thread=False)
        except sqlite3.OperationalError:
            os.makedirs(os.path.dirname(path))
            con = sqlite3.connect(path, timeout=60, check_same_thread=False)
        con.isolation_level = None

        self.close = con.close
        self.cursor = con.cursor()

        try:
            self.cursor.execute("CREATE TABLE IF NOT EXISTS archive " "(entry PRIMARY KEY) WITHOUT ROWID")
        except sqlite3.OperationalError:
            # fallback for missing WITHOUT ROWID support (#553)
            self.cursor.execute("CREATE TABLE IF NOT EXISTS archive " "(entry PRIMARY KEY)")

    def check(self, relativePath):
        """Return True if the item described by relativePath exists in archive, after converting to realpath"""
        key = os.path.realpath(relativePath)
        self.cursor.execute("SELECT 1 FROM archive WHERE entry=? LIMIT 1", (key,))
        return self.cursor.fetchone()

    def add(self, relativePath):
        """Add item described by 'relativePath' to archive, after converting to realpath"""
        key = os.path.realpath(relativePath)
        self.cursor.execute("INSERT OR IGNORE INTO archive VALUES (?)", (key,))

    def appendToListIfNotInArchive(self, list: list, path: str, originalsize: int, args: apns) -> list:
        if self.check(path):  # item is in archive, don't do anything
            if args.v:
                cprint(f"{path} has been previously processed", "blue")
        else:
            list.append((path, originalsize))
        return list


async def smartProcessFile(
    inputFile: str,
    inputSize: int,
    args: apns,
    historyObj: CompressHistory,
):
    assert os.path.isfile(inputFile), f"ERROR: missing file {inputFile}"
    assert os.access(inputFile, os.W_OK), f"ERROR: file {inputFile} is not writeable"

    if args.jpg:
        inputFile = await convertImageMagick(inputFile, toFmt="jpg", args=args)
    elif args.png:
        inputFile = await convertImageMagick(inputFile, toFmt="png", args=args)
    elif args.heif:
        inputFile = convertHEIF(inputFile, args=args)

    fileExt = os.path.splitext(inputFile)[1].lower()
    if fileExt == ".png":
        if args.lossy:
            result = await pngquant(inputFile, args=args)
            if args.v:
                print(result)
        result = await oxipng(inputFile, args=args)
        if args.v:
            print(result)
    elif fileExt == ".gif":
        result = await gifsicle(inputFile, args=args)
        if args.v:
            print(result)
    elif fileExt == ".jpg" or fileExt == ".jpeg":
        if args.lossy:
            result = await cjpeg(inputFile, args=args)
            if args.v:
                print(result)
        result = await mozjpg(inputFile, args=args)
        if args.v:
            print(result)
    elif fileExt == ".heif" or fileExt == ".heic":
        pass  # no aditional operations here for now since we aren't recompressing HEIF images
    else:
        assert False, f"error, this should not be here, no processing, unknown filetime for {inputFile}"

    if not os.path.isfile(inputFile):
        cprint(f"WARNING: missing expected output {inputFile}, returning nothing", "yellow")
        return inputSize, inputFile

    newSize = os.path.getsize(inputFile)
    if not args.q:
        if newSize < inputSize:
            cprint(
                f"Compressed {inputFile} by {1 - newSize / inputSize:.2%} & saved: {bytes2human(inputSize-newSize)}",
                "green",
            )
        else:
            cprint(f"No space saved for {inputFile}, keeping original.", "red")

    if historyObj:
        historyObj.add(inputFile)

    return newSize, inputFile

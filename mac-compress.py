#!/usr/bin/env python3.9
# from enum import Enum, unique
# import shutil
import argparse
import os
import sys
import time
from click import echo, style
import asyncio
from limiter import get_limiter, limit

# global for if we want to use lossy compression
doLossy = True


def cprint(text: str, color: str = "white"):
    echo(style(text, fg=color))


async def subProcessToStr(args: list, printCmd: bool = True, stdErrIsErr: bool = True) -> str:
    calledCmd = " ".join(args)
    if printCmd:
        print(calledCmd)
    proc = await asyncio.create_subprocess_exec(
        args[0], *args[1:], stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE
    )
    stdout, stderr = await proc.communicate()
    if stderr and stdErrIsErr:
        cprint(f"*stderr for {calledCmd}]\n{stderr.decode('utf-8')}*", "yellow")
        raise TypeError
    if not stdErrIsErr:
        print(stderr.decode("utf-8"))
    return stdout.decode("utf-8")


# We want to rate limit calls to sumo here, they allow 4 calls/second
# API allows 4 calls/second but only allow 3 slots to free per second for safety
# consume 1 means each call here uses 1 slot (which is adjustable for other cases)
limit_operations = limit(get_limiter(rate=3, capacity=4), consume=1)


@limit_operations
async def metricsTypeTest(deploymentName: str):
    # tplt = metricTypeTemplate.replace("DEPLOYMENTNAMEREPLACE", deploymentName)
    # async with httpx.AsyncClient(timeout=5) as client:
    #     response = await client.post("https://api.us2.sumologic.com/api/v1/metricsQueries")
    # try:
    #     response.raise_for_status()
    # except httpx.HTTPStatusError as exc:
    #     cprint(f"Error response {exc.response.status_code} while requesting {exc.request.url!r}.", "red")
    #     raise exc
    # return response.json()["queryResult"]

    # async def getDeploymentsWithMetricsInfoForNS(namespace: str) -> list:
    #     deploymentList = await getDeploymentsForNS(namespace)
    #     metricsTypes = await asyncio.gather(*(metricsTypeTest(i) for i in deploymentList))
    #     typeDict = dict(zip(deploymentList, metricsTypes))
    #     return typeDict

    # async def buildDictMapping() -> dict:
    #     listOfNamespaces = await getListOfNamespaces()
    #     assert len(listOfNamespaces) > 0, "No matching namespaces returned, are you in staging rather than prod context?"
    #     deploymentLists = await asyncio.gather(*(getDeploymentsWithMetricsInfoForNS(i) for i in listOfNamespaces))
    #     return dict(zip(listOfNamespaces, deploymentLists))
    pass


# ZopfliPNG, a Portable Network Graphics (PNG) image optimizer.
# Mostly lossless
async def zopflipng(input: str) -> str:
    args = [
        "/opt/homebrew/bin/zopflipng",
        "-y",  # allow overwrite so we can update-in-place
        "--lossy_transparent",  # remove colors behind alpha channel 0. No visual difference
        input,
        input,  # will output to input
    ]
    if doLossy:
        args.append("--lossy_8bit")  # convert 16-bit per channel image to 8-bit per channel
    return await subProcessToStr(args)


# pngquant is a util for lossy compression of PNG images.
# very lossy - uses a reduced number of colors
async def pngquant(input: str) -> str:
    if not doLossy:
        warnMessage = "pngquant is lossy and you requested lossles"
        cprint(warnMessage, "yellow")
        return warnMessage
    args = [
        "/opt/homebrew/bin/pngquant",
        "--force",  # allow overwrite so we can update-in-place
        "--skip-if-larger",  # only save converted files if they're smaller than original
        "--strip",  # remove optional metadata (default on Mac)
        "--quality=85-90",  # don't save below min, use fewer colors below max (0-100)
        "--output",
        input,  # will output to input
        "--",  # Stops processing of arguments
        input,
    ]
    return await subProcessToStr(args)


# Oxipng is a multithreaded lossless PNG compression optimizer
async def oxipng(input: str) -> str:
    args = [
        "/opt/homebrew/bin/oxipng",
        "--preserve",  # Preserve file attributes if possible
        "-s",  # Strip safely-removable metadata objects
        "--alpha",  # Perform additional alpha optimizations
        "--interlace",
        "0",  # disable interlacing
        # "--zopfli",  # Use the slower but better compressing Zopfli algorithm
        input,  # will output to input
    ]
    return await subProcessToStr(args, stdErrIsErr=False)


async def main():
    parser = argparse.ArgumentParser(description="compress images")
    parser.add_argument("--lossy", action="store_true")
    parser.add_argument("files", metavar="F", type=str, nargs="+", help="an integer for the accumulator")
    args = parser.parse_args()
    global doLossy
    doLossy = args.lossy

    if doLossy:
        cprint(f"Lossy processing enabled: {sys.argv}", color="yellow")
    else:
        cprint(f"Lossy processing off: {sys.argv}", color="green")

    for inputFile in args.files:
        assert os.path.isfile(inputFile), f"ERROR: missing file {inputFile}"
        # do lossy processing first
        if doLossy:
            result = await pngquant(inputFile)
        # lossless
        result = await oxipng(inputFile)
        # result = await zopflipng(inputFile)  # oxipng can do the same thing...
        print(result)


if __name__ == "__main__":
    print(f"Started at {time.strftime('%X')}")
    asyncio.run(main())
    print(f"Ended at {time.strftime('%X')}")

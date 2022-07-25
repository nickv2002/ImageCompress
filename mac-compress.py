#!/usr/bin/env python3.9
# from enum import Enum, unique
# import shutil
import time
from click import echo, style
import asyncio
from limiter import get_limiter, limit


def cprint(text: str, color: str = "white"):
    echo(style(text, fg=color))


async def subProcessToStr(args: list) -> str:
    proc = await asyncio.create_subprocess_exec(
        args[0], *args[1:], stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE
    )
    stdout, stderr = await proc.communicate()
    if stderr:
        cprint(f"[stderr for {args}]\n{stderr.decode('utf-8')}", "yellow")
        raise TypeError
    return stdout.decode("utf-8")


# We want to rate limit calls to sumo here, they allow 4 calls/second
# API allows 4 calls/second but only allow 3 slots to free per second for safety
# consume 1 means each call here uses 1 slot (which is adjustable for other cases)
limit_operations = limit(get_limiter(rate=3, capacity=4), consume=1)


@limit_operations
async def metricsTypeTest(deploymentName: str):
    pass
    # tplt = metricTypeTemplate.replace("DEPLOYMENTNAMEREPLACE", deploymentName)
    # async with httpx.AsyncClient(timeout=5) as client:
    #     response = await client.post("https://api.us2.sumologic.com/api/v1/metricsQueries")
    # try:
    #     response.raise_for_status()
    # except httpx.HTTPStatusError as exc:
    #     cprint(f"Error response {exc.response.status_code} while requesting {exc.request.url!r}.", "red")
    #     raise exc
    # return response.json()["queryResult"]


async def getDeploymentsForNS(namespace: str) -> list:
    args = [
        "/opt/homebrew/bin/kubectl",
        "get",
        "deployment",
        "--namespace",
        f"{namespace}",
        "-o=jsonpath='{.items[*].metadata.name}'",
    ]
    deploymentList = (await subProcessToStr(args)).replace("'", "").split()
    for item in deploymentList:
        if "worker" in item or "amqp-prom" in item:
            cprint(f"Skipping deployment: {namespace} - {item}", color="yellow")
            deploymentList.remove(item)
    deploymentList.sort()
    return deploymentList


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


async def main():
    # buildDict = await buildDictMapping()
    cprint("Adding panel for deployment:", color="green")


if __name__ == "__main__":
    print(f"Started at {time.strftime('%X')}")
    asyncio.run(main())
    print(f"Ended at {time.strftime('%X')}")

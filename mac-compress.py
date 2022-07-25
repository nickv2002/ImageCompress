#!/usr/bin/env python3.9
from enum import Enum, unique
import shutil
import json
import time
from click import echo, style
import httpx
import asyncio
from limiter import get_limiter, limit


def cprint(text: str, color: str = "white", fg: str = None):
    if fg:
        color = fg
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


@unique
class MetricType(str, Enum):
    django: str = "django"
    node: str = "node"
    unknown: str = "unknown"


async def getListOfNamespaces() -> list:
    # get a list of namespaces from kubens and prune them to just the stuff we care about
    listOfNamespaces = []
    for namespace in (await subProcessToStr([shutil.which("kubens")])).split():
        if (
            (not namespace.endswith("prod"))
            or (namespace == "observability-prod")
            or (namespace == "retool-prod")
            or (namespace == "slurper-prod")
            or (namespace == "split-synchronizer-prod")
            # or not (namespace.startswith("c"))  # TODO remove after testing
        ):
            continue
        listOfNamespaces.append(namespace)
    return listOfNamespaces


# We want to rate limit calls to sumo here, they allow 4 calls/second
# API allows 4 calls/second but only allow 3 slots to free per second for safety
# consume 1 means each call here uses 1 slot (which is adjustable for other cases)
limit_downloads = limit(get_limiter(rate=3, capacity=4), consume=1)


@limit_downloads
async def metricsTypeTest(deploymentName: str) -> Enum:
    tplt = metricTypeTemplate.replace("DEPLOYMENTNAMEREPLACE", deploymentName)

    async with httpx.AsyncClient(timeout=5) as client:
        response = await client.post(
            "https://api.us2.sumologic.com/api/v1/metricsQueries", headers=sumoAuthHeaders, data=tplt
        )
    try:
        response.raise_for_status()
    except httpx.HTTPStatusError as exc:
        cprint(f"Error response {exc.response.status_code} while requesting {exc.request.url!r}.", "red")
        raise exc
    jr = response.json()["queryResult"]

    if jr == []:
        cprint(f"No query result for {deploymentName}", "yellow")
        return MetricType.unknown
    elif jr[0]["rowId"] == "A":
        return MetricType.django
    elif jr[0]["rowId"] == "B":
        return MetricType.node
    cprint(f"No matching for {deploymentName}", "red")
    return MetricType.unknown


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


async def getDeploymentsWithMetricsInfoForNS(namespace: str) -> list:
    deploymentList = await getDeploymentsForNS(namespace)
    metricsTypes = await asyncio.gather(*(metricsTypeTest(i) for i in deploymentList))
    typeDict = dict(zip(deploymentList, metricsTypes))
    return typeDict


async def buildDictMapping() -> dict:
    listOfNamespaces = await getListOfNamespaces()
    assert len(listOfNamespaces) > 0, "No matching namespaces returned, are you in staging rather than prod context?"
    deploymentLists = await asyncio.gather(*(getDeploymentsWithMetricsInfoForNS(i) for i in listOfNamespaces))
    return dict(zip(listOfNamespaces, deploymentLists))


async def main():
    buildDict = await buildDictMapping()
    xLoc = yLoc = 0  # this is to track the position of our panels
    # build list of namespaces
    for namespace in buildDict:
        shouldAddHeader = True
        for deployment in buildDict[namespace]:
            if shouldAddHeader:  # we only add header text here in case deploymentList is all skipped
                cprint(f"Adding header for {namespace} ({xLoc},{yLoc})", color="blue")
                # add a heading text box and adjust our active location
                templateBase["layout"]["layoutStructures"].append(
                    {
                        "key": f"{namespace}_Title_Heading",
                        "structure": f'{{"height":1,"width":24,"x":{xLoc},"y":{yLoc}}}',
                    }
                )
                templateBase["panels"].append(
                    json.loads(sumoDashDict["title"].replace("REPLACENAMESPACENAME", namespace))
                )
                shouldAddHeader = False
                yLoc += 1

            if xLoc >= 24:
                # print("adjust our position down for mod 24")
                xLoc = 0
                yLoc += 8

            cprint(f"Adding panel for deployment: {deployment} ({xLoc},{yLoc})", color="green")
            templateBase["layout"]["layoutStructures"].append(
                {
                    "key": f"{deployment}_Response_Codes",
                    "structure": f'{{"height":8,"width":8,"x":{xLoc},"y":{yLoc}}}',
                }
            )
            panelTemplate = sumoDashDict[buildDict[namespace][deployment]]
            templateBase["panels"].append(json.loads(panelTemplate.replace("REPLACEDEPLOYMENTNAME", deployment)))

            # print("adjust our position over")
            xLoc += 8

        if len(buildDict[namespace]) > 0:
            # print("position for the next header")
            xLoc = 0
            yLoc += 8

    with open(".secrets/testoutput.json", "w", encoding="utf8") as json_file:
        json.dump(templateBase, json_file, ensure_ascii=True, indent=4)

    response = httpx.put(
        "https://api.us2.sumologic.com/api/v2/dashboards/TCUjWUwn1Vehamsri3sxHItZ1AZYNT1llzwf56VVVslWFdrfeW9XXwu3iEhv",
        headers={"Content-Type": "application/json", "Authorization": authkey},
        data=json.dumps(templateBase),
    )
    try:
        response.raise_for_status()
    except httpx.HTTPStatusError as exc:
        cprint(f"Error response {exc.response.status_code} while requesting {exc.request.url!r}.", "red")
    else:
        cprint("Completed successfully", color="blue")


if __name__ == "__main__":
    print(f"Started at {time.strftime('%X')}")

    # Start by reading in our templates and secret auth key
    # We use globals here so we can use async functions easily without passing around variables
    global sumoDashDict
    sumoDashDict = {}
    with open("templatePanelEmpty.json") as file:
        sumoDashDict[MetricType.unknown] = file.read()
    with open("templatePanelDjango.json") as file:
        sumoDashDict[MetricType.django] = file.read()
    with open("templatePanelNode.json") as file:
        sumoDashDict[MetricType.node] = file.read()
    with open("templateTitle.json") as file:
        sumoDashDict["title"] = file.read()
    with open("templateBase.json") as fJson:
        global templateBase
        templateBase = json.load(fJson)
    with open("templateMetricType.json") as file:
        global metricTypeTemplate
        metricTypeTemplate = file.read()
    with open(".secrets/authKey") as file:
        authkey = file.read().strip()
        global sumoAuthHeaders
        sumoAuthHeaders = {"Content-Type": "application/json", "Authorization": authkey}

    asyncio.run(main())
    print(f"Ended at {time.strftime('%X')}")

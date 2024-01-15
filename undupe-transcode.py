#!/usr/bin/env python3
import os
import sys
import glob
import subprocess
import shlex
import shutil
import itertools


def multiple_file_types(*patterns):
    return itertools.chain.from_iterable(
        glob.iglob(pattern, recursive=True) for pattern in patterns
    )


def convert_bytes(num):
    """
    this function will convert bytes to MB.... GB... etc
    """
    for x in ["bytes", "KB", "MB", "GB", "TB"]:
        if num < 1024.0:
            return "%3.1f %s" % (num, x)
        num /= 1024.0


def get_first_line_from_command(command: str) -> str:
    command_parts = shlex.split(command)
    output = subprocess.run(
        command_parts, capture_output=True, text=True, universal_newlines=True
    )
    for line in output.stdout.split("\n"):
        return line.strip()
    raise TypeError(f"ERROR: we expected output but `{command}` had none")


if __name__ == "__main__":
    FFPROBE = "/opt/homebrew/bin/ffprobe"
    TVPATH = "/usr/local/bin/transcode-video"
    MAXCOMPRESSRATIO = (
        0.95  # size of H.265 file over size of original file, max ratio allowed
    )
    MAXSECONDSSHIFT = 2.5  # max seconds the new file is alowed to be shorter or longer than the old file
    REPLACEIFMP4ORMKV = False

    assert os.path.isfile(
        FFPROBE
    ), f"ERROR: please install ffmpeg which includes a seperate ffprobe binary and fix the listed path: {FFPROBE}"
    assert os.path.isfile(
        TVPATH
    ), f"ERROR: please install transcode-video and fix the listed path: {TVPATH}"
    assert (
        0.25 <= MAXCOMPRESSRATIO <= 1.1
    ), f"ERROR: value given for MAXCOMPRESSRATIO is unreasonable: {MAXCOMPRESSRATIO} (it should probably be somewhere between .95 & .75)"

    for vidFile in multiple_file_types(
        "**/*.mov",
        "**/*.mp4",
        "**/*.mkv",
        "**/*.m4v",
        "**/*.avi",
        "**/*.wmv",
        "**/*.webm",
        "**/*.flv",
    ):
        already265 = False
        foundVideoType = False
        if vidFile[-3:] == "mp4" and REPLACEIFMP4ORMKV:
            vidFile265 = ".".join(vidFile.split(".")[:-1]) + ".h265.mp4"
        else:
            vidFile265 = ".".join(vidFile.split(".")[:-1]) + ".h265.mkv"
        poorTranscodeLogPath = os.path.join(
            os.path.split(vidFile)[0], ".poorTranscodeLog"
        )
        badTranscodeLogPath = os.path.join(
            os.path.split(vidFile)[0], ".badTranscodeLog"
        )

        if os.path.isfile(poorTranscodeLogPath):
            lines = [line.strip("\n") for line in open(poorTranscodeLogPath)]
            if vidFile265 in lines:
                # print(f'Skipping {vidFile} because it had a poor H.265 encode (A)')
                continue
            elif vidFile in lines:
                print(
                    f"Skipping {vidFile} because this is the poor H.265 encode, perhaps delete this file? (B)"
                )
                continue

        if os.path.isfile(badTranscodeLogPath):
            lines = [line.strip("\n") for line in open(badTranscodeLogPath)]
            if vidFile265 in lines:
                # print(f'Skipping {vidFile} because it had a bad H.265 encode (C)')
                continue
            elif vidFile in lines:
                print(
                    f"Skipping {vidFile} because this is the bad H.265 encode, perhaps delete this file? (D)"
                )
                continue

        if "hevc" in vidFile or "HEVC" in vidFile or "265" in vidFile:
            already265 = True
        else:
            print(f"Processing {vidFile}")
            command = shlex.split(f'{FFPROBE} -hide_banner "{vidFile}"')
            output = subprocess.run(
                command, capture_output=True, text=True, universal_newlines=True
            )
            for line in output.stderr.split("\n"):
                line = line.strip()
                if line.startswith("Stream") and "Video" in line:
                    foundVideoType = True
                    if "hevc" in line:
                        already265 = True
            if not foundVideoType:
                print("Command failed:")
                for i in command:
                    print(i + " ", end="")
                print("")
                sys.exit()

        if already265:
            print(
                f"--------------------\nSkipping {vidFile} which is alredy H.265 (this is probably good)"
            )
            continue
        else:
            print(f"--------------------\nProcessing {vidFile} to {vidFile265}")
            command = shlex.split(
                f'{TVPATH} -q --encoder vt_h265 --target small -o "{vidFile265}" "{vidFile}"'
            )
            print(command)
            subprocess.run(command)

        # ---------------------------------------------------------------------
        # Compare resulting file sizes
        # technically this could be part of the previous statement after we run the encode but this way it's resumable
        if os.path.isfile(vidFile) and os.path.isfile(vidFile265):
            vidFileSize = os.path.getsize(vidFile)
            vidFile265Size = os.path.getsize(vidFile265)
            vidFilesSizeDiff = vidFileSize - vidFile265Size

            if vidFile265Size >= vidFileSize * MAXCOMPRESSRATIO:
                print(f"Poor encode, not sufficiently smaller: {vidFile265} (E)")
                with open(poorTranscodeLogPath, "a") as pTLO:
                    pTLO.write(vidFile265 + "\n")
                os.remove(vidFile265)
            else:
                try:
                    # first check their length with ffprobe, sexagesimal give an HOURS:MM:SS.MICROSECONDS string vs a raw second count with decimals
                    vidFile265LengthString = get_first_line_from_command(
                        f'{FFPROBE} -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 -sexagesimal "{vidFile265}"'
                    )
                    vidFile265Length = float(
                        get_first_line_from_command(
                            f'{FFPROBE} -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "{vidFile265}"'
                        )
                    )
                except ValueError:
                    # we probably have a bad conversion here somwhere, we should be able to read this info about our new conversion
                    print(
                        f"Bad encode, error getting video length from {vidFile265} (F)"
                    )
                    with open(poorTranscodeLogPath, "a") as pTLO:
                        pTLO.write(vidFile265 + "\n")
                    os.remove(vidFile265)
                    continue

                try:
                    vidFileLengthString = get_first_line_from_command(
                        f'{FFPROBE} -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 -sexagesimal "{vidFile}"'
                    )
                    vidFileLength = float(
                        get_first_line_from_command(
                            f'{FFPROBE} -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "{vidFile}"'
                        )
                    )
                except ValueError:
                    # we probably have a bad conversion here somwhere
                    print(
                        f"Bad source, error getting video length from {vidFile}, check file in player (G)"
                    )
                    with open(poorTranscodeLogPath, "a") as pTLO:
                        pTLO.write(vidFile265 + "\n")
                    continue

                if abs(vidFile265Length - vidFileLength) > MAXSECONDSSHIFT:
                    print(
                        f"Bad encode, orginal video length: {vidFileLengthString.split('.')[0]}, {vidFile265} vidFile265Length: {vidFile265LengthString.split('.')[0]}, more than max diff {MAXSECONDSSHIFT} (H)"
                    )
                    with open(poorTranscodeLogPath, "a") as pTLO:
                        pTLO.write(vidFile265 + "\n")
                    os.remove(vidFile265)
                    continue
                else:
                    print(
                        f"Saved {convert_bytes(vidFilesSizeDiff)} ({int(100*(vidFilesSizeDiff)/float(vidFileSize))}%) by converting to {vidFile265}"
                    )
                    os.remove(vidFile)
                    if REPLACEIFMP4ORMKV:
                        shutil.move(vidFile265, vidFile)
                        print(f"Also moved {vidFile265} to {vidFile}")

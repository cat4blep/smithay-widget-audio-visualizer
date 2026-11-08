"""Exercise the native widget over the same three file descriptors as the host."""

import math
import os
from pathlib import Path
import selectors
import struct
import subprocess
import sys
import tempfile
from contextlib import contextmanager


ACK = b"SMD1"
ACCENT = bytes((42, 210, 110, 255))
NEW_ACCENT = bytes((232, 132, 25, 255))
EMPTY = bytes((24, 66, 44, 255))
MAX_PIXELS = 512 * 256 * 4


def frame(width, height, target, revision, *, flags=0, tone=None):
    if tone is None:
        rate = 0
        samples = ()
    else:
        rate = 16000
        samples = tuple(round(math.sin(math.tau * tone * i / rate) * 18000)
                        for i in range(512))
    header = struct.pack(
        "<IIIIQiiIIII", 1, width, height, target, 0, -1, -1,
        flags, revision, rate, len(samples),
    )
    return header + struct.pack(f"<{len(samples)}h", *samples)


@contextmanager
def run(executable, environment, buffer):
    with subprocess.Popen(
        [executable, "--environment", environment],
        stdin=subprocess.PIPE, stdout=buffer, stderr=subprocess.PIPE, bufsize=0,
    ) as child:
        try:
            yield child
        finally:
            if child.poll() is None:
                child.kill()


def receive(child, buffer, width, height):
    with selectors.DefaultSelector() as selector:
        selector.register(child.stderr, selectors.EVENT_READ)
        assert selector.select(3), "Timed out waiting for frame acknowledgement"
    assert child.stderr.read(4) == ACK
    assert os.lseek(buffer, 0, os.SEEK_CUR) == 19, "The frame changed the shared cursor"
    size = width * height * 4
    pixels = os.pread(buffer, size, 0)
    assert len(pixels) == size
    return {pixels[i:i + 4] for i in range(0, size, 4)}


def exercise(executable, path, buffer):
    environment = bytearray(64)
    struct.pack_into("<II", environment, 0, 1, 2)
    environment[16:20] = ACCENT
    environment[24:32] = bytes((83, 84, 90, 255, 48, 48, 53, 255))
    environment[36:40] = EMPTY
    path.write_bytes(environment)
    os.ftruncate(buffer, MAX_PIXELS)
    os.lseek(buffer, 19, os.SEEK_SET)

    with run(executable, path, buffer) as child:
        initial = frame(58, 30, 0, 2, flags=1, tone=1000)
        child.stdin.write(initial[:23])
        with selectors.DefaultSelector() as selector:
            selector.register(child.stderr, selectors.EVENT_READ)
            assert not selector.select(0.05), "Acknowledged a partial header"
        child.stdin.write(initial[23:])
        assert ACCENT in receive(child, buffer, 58, 30)

        with path.open("r+b", buffering=0) as file:
            os.pwrite(file.fileno(), struct.pack("<I", 3), 4)
            os.pwrite(file.fileno(), NEW_ACCENT, 16)
            os.pwrite(file.fileno(), struct.pack("<I", 4), 4)

        child.stdin.write(frame(58, 30, 0, 2, tone=1000))
        assert ACCENT in receive(child, buffer, 58, 30)
        child.stdin.write(frame(320, 116, 1, 4, tone=5000))
        colors = receive(child, buffer, 320, 116)
        assert NEW_ACCENT in colors and ACCENT not in colors
        child.stdin.write(frame(512, 256, 1, 4))
        colors = receive(child, buffer, 512, 256)
        assert EMPTY in colors and NEW_ACCENT not in colors

        child.stdin.close()
        assert child.wait(timeout=3) == 0
        assert child.stderr.read() == b""

    malformed = (
        frame(58, 30, 0, 4)[:23],
        frame(58, 30, 0, 4, tone=1000)[:-2],
        frame(513, 30, 0, 4),
    )
    for record in malformed:
        with run(executable, path, buffer) as child:
            child.stdin.write(record)
            child.stdin.close()
            assert child.wait(timeout=3) != 0
            assert child.stderr.read() == b""


if __name__ == "__main__":
    binary = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix="smithay-visualizer-") as directory:
        fd = os.memfd_create("widget-pixels", os.MFD_CLOEXEC)
        try:
            exercise(binary, Path(directory) / "environment", fd)
        finally:
            os.close(fd)
    print("Native widget protocol passed")

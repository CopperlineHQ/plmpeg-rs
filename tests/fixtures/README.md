# Streaming conformance fixture

`streaming-48x32.m1v.b64` is an eight-frame MPEG-1 elementary stream generated
with FFmpeg 8.0.1 from its synthetic `testsrc2` source:

```sh
ffmpeg -f lavfi -i 'testsrc2=size=48x32:rate=25:duration=0.32' \
  -c:v mpeg1video -qscale:v 4 -g 6 -bf 2 -f mpeg1video streaming-48x32.m1v
```

`streaming-48x32.ref.yuv.b64` is the corresponding FFmpeg decode in planar
YUV 4:2:0 display order. The files are base64 encoded so the source tree and
patch-based development tools can treat the small binary fixtures as text.

SHA-256 before base64 encoding:

```text
2e9d743603dd43a75c11ec0dd9fe3e94034101a0305776484c1dc6cd5779c3b6  streaming-48x32.m1v
e2e55a804cc60f7b3ed2c0ba77cb742c583d078d6b0165b8a30b320c1220ba4b  streaming-48x32.ref.yuv
```

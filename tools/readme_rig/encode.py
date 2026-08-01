#!/usr/bin/env python3
"""Turn a directory of screencast frames into the README's clip.

Two things make this different from `ffmpeg -framerate N`:

* **The page does not paint at a frame rate.** It repaints once or twice a turn
  while the game runs and at whatever the machine can do while the camera moves,
  so a constant-rate encode pays for hundreds of duplicate frames and still gets
  the timing wrong. `mpdecimate` finds the distinct pictures and `-frame_pts 1`
  writes each survivor's *original index* as its filename, so its wall-clock
  timestamp can be looked back up.
* **A whole game is twenty minutes and a README clip is three.** Each distinct
  picture is given the time it really held the screen, *capped*: a late-game turn
  that sat still for four seconds is cut to the cap, while a camera move already
  painting every 40 ms is under the cap and plays at its real speed. So the
  timelapse compresses only the waiting, and the choreography stays real time.

Usage: encode.py <framedir> <outdir> [--cap 0.09] [--width 1600]
"""
import argparse
import json
import os
import shutil
import subprocess
import sys


def run(cmd):
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        sys.stderr.write(result.stderr[-4000:])
        raise SystemExit(f"failed: {' '.join(cmd[:6])}…")
    return result.stderr


def stamps(framedir):
    out = {}
    with open(os.path.join(framedir, "frames.jsonl")) as handle:
        for line in handle:
            entry = json.loads(line)
            out[entry["index"]] = entry["ts"]
    return out


def decimate(framedir, keptdir, hi, lo, frac):
    shutil.rmtree(keptdir, ignore_errors=True)
    os.makedirs(keptdir, exist_ok=True)
    run(["ffmpeg", "-nostdin", "-v", "error", "-framerate", "100", "-start_number", "0",
         "-i", os.path.join(framedir, "%06d.jpg"),
         "-vf", f"mpdecimate=hi={hi}:lo={lo}:frac={frac}",
         "-fps_mode", "vfr", "-frame_pts", "1",
         os.path.join(keptdir, "%06d.jpg")])
    return sorted(int(name[:-4]) for name in os.listdir(keptdir) if name.endswith(".jpg"))


def durations(kept, ts, cap, floor, tail):
    """Real time each distinct picture held the screen, capped."""
    out = []
    for position, index in enumerate(kept):
        if position + 1 < len(kept):
            held = ts[kept[position + 1]] - ts[index]
        else:
            held = tail
        out.append(min(max(held, floor), cap))
    return out


def beat_windows(framedir, pad=1.0):
    """Wall-clock intervals during which a scripted camera move was running.

    The recorder writes one `beat` note when a beat starts and one `step` note
    as each of its moves finishes, so a beat's window runs from its own note to
    its last step's. Everything outside those windows is the game simply
    running, which is the part that should be a timelapse — the moves themselves
    have to keep their real timing or a fourteen-second turn of the globe plays
    as a twitch."""
    path = os.path.join(framedir, "events.jsonl")
    if not os.path.exists(path):
        return []
    events = []
    for line in open(path):
        try:
            events.append(json.loads(line))
        except ValueError:
            pass
    windows, start = [], None
    for event in events:
        if event["kind"] == "beat":
            if start is not None:
                windows.append(start)
            start = [event["t"] / 1000 - pad, event["t"] / 1000]
        elif event["kind"] == "step" and start is not None:
            start[1] = event["t"] / 1000 + pad
    if start is not None:
        windows.append(start)
    return windows


def in_any(windows, when):
    return any(low <= when <= high for low, high in windows)


def timelapse(kept, spans, ts, windows, stride, beat_frame):
    """Keep every picture inside a scripted move; keep every `stride`-th picture
    outside one and hold it for a fixed short beat. The alternative — one cap for
    the whole take — has to be set by the longest thing in it, so either the game
    runs for six minutes or the camera work plays at three times speed."""
    out_kept, out_spans, skipped = [], [], 0
    for index, span in zip(kept, spans):
        if in_any(windows, ts[index]):
            out_kept.append(index)
            out_spans.append(span)
            skipped = 0
        else:
            if skipped % stride == 0:
                out_kept.append(index)
                out_spans.append(beat_frame)
            skipped += 1
    return out_kept, out_spans


def write_concat(path, keptdir, kept, spans, stride=1):
    """The concat demuxer only honours a `duration` when another entry follows
    it, so the closing frame is named twice."""
    lines = ["ffconcat version 1.0"]
    # Keeping every Nth picture has to keep the time too, or the clip runs N
    # times short and the pacing of the whole game changes with the stride.
    picked = [(kept[start], sum(spans[start:start + stride]))
              for start in range(0, len(kept), stride)]
    for index, span in picked:
        lines.append(f"file '{os.path.join(keptdir, f'{index:06d}.jpg')}'")
        lines.append(f"duration {span:.4f}")
    lines.append(f"file '{os.path.join(keptdir, f'{picked[-1][0]:06d}.jpg')}'")
    with open(path, "w") as handle:
        handle.write("\n".join(lines) + "\n")
    return sum(span for _, span in picked)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("framedir")
    parser.add_argument("outdir")
    parser.add_argument("--cap", type=float, default=0.20)
    parser.add_argument("--floor", type=float, default=0.02)
    parser.add_argument("--tail", type=float, default=2.0)
    parser.add_argument("--stride", type=int, default=5,
                        help="keep every Nth distinct picture outside a scripted move")
    parser.add_argument("--lapse", type=float, default=0.055,
                        help="seconds each timelapsed picture is held")
    parser.add_argument("--pad", type=float, default=1.0)
    parser.add_argument("--width", type=int, default=1600)
    parser.add_argument("--gif-width", type=int, default=900)
    parser.add_argument("--gif-frame", type=float, default=0.09)
    parser.add_argument("--gif-stride", type=int, default=13)
    parser.add_argument("--gif-colors", type=int, default=96)
    parser.add_argument("--hi", type=int, default=1000)
    parser.add_argument("--lo", type=int, default=500)
    parser.add_argument("--frac", type=float, default=0.002)
    args = parser.parse_args()

    os.makedirs(args.outdir, exist_ok=True)
    ts = stamps(args.framedir)
    keptdir = os.path.join(args.outdir, "kept")
    kept = decimate(args.framedir, keptdir, args.hi, args.lo, args.frac)
    total_raw = ts[max(ts)] - ts[min(ts)]
    print(f"{len(ts)} frames over {total_raw:.1f}s → {len(kept)} distinct")

    spans = durations(kept, ts, args.cap, args.floor, args.tail)
    windows = beat_windows(args.framedir, args.pad)
    if windows and args.stride > 1:
        kept, spans = timelapse(kept, spans, ts, windows, args.stride, args.lapse)
        print(f"{len(windows)} scripted windows kept at real time, "
              f"the rest every {args.stride} at {args.lapse}s → {len(kept)} pictures")
    concat = os.path.join(args.outdir, "clip.ffconcat")
    length = write_concat(concat, keptdir, kept, spans)
    print(f"mp4 length {length:.1f}s")

    mp4 = os.path.join(args.outdir, "exhibition.mp4")
    # Name the colour matrix or x264 stamps bt470bg — 625-line PAL SD — on an HD
    # clip, and players that honour the tag and players that assume bt709 then
    # render different colours. Keep it full range: the source frames are.
    run(["ffmpeg", "-nostdin", "-v", "error", "-y", "-safe", "0", "-f", "concat",
         "-i", concat,
         "-vf", f"scale={args.width}:-2:flags=lanczos:out_color_matrix=bt709,fps=30,format=yuv420p",
         "-t", f"{length:.3f}",
         "-c:v", "libx264", "-preset", "slow", "-crf", "20",
         # The container flags alone leave primaries and transfer *unknown* once
         # the pixel format goes full-range (yuvj420p); x264 has to be told as
         # well, or only the matrix survives into the stream.
         "-x264-params", "colorprim=bt709:transfer=bt709:colormatrix=bt709:fullrange=on",
         "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
         "-color_range", "pc", "-movflags", "+faststart", mp4])

    # The GIF is a different job from the mp4 and cannot be the same encode
    # scaled down. It is the README's *poster* — it has to autoplay for every
    # reader on every renderer and stay a few megabytes — so it drops time
    # rather than compressing it: every Nth distinct picture, each held the same
    # short beat. Keeping the real durations here would make a three-minute clip
    # a thirty-megabyte GIF, and merging the dropped frames' time back in would
    # keep it three minutes long.
    gif_kept = kept[::args.gif_stride]
    gif_spans = [args.gif_frame] * len(gif_kept)
    gif_concat = os.path.join(args.outdir, "gif.ffconcat")
    gif_length = write_concat(gif_concat, keptdir, gif_kept, gif_spans)
    palette = os.path.join(args.outdir, "palette.png")
    run(["ffmpeg", "-nostdin", "-v", "error", "-y", "-safe", "0", "-f", "concat",
         "-i", gif_concat, "-vf",
         f"scale={args.gif_width}:-2:flags=lanczos,palettegen=stats_mode=diff:max_colors={args.gif_colors}", palette])
    gif = os.path.join(args.outdir, "exhibition.gif")
    run(["ffmpeg", "-nostdin", "-v", "error", "-y", "-safe", "0", "-f", "concat",
         "-i", gif_concat, "-i", palette, "-lavfi",
         f"scale={args.gif_width}:-2:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3",
         "-fps_mode", "passthrough", "-loop", "0", gif])

    for name in (mp4, gif):
        print(f"{os.path.basename(name)}: {os.path.getsize(name) / 1e6:.2f} MB")
    print(f"gif length {gif_length:.1f}s")


if __name__ == "__main__":
    main()

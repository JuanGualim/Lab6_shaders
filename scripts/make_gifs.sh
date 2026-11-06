#!/usr/bin/env bash
# Genera las capturas y los GIF del README.
#   1. cargo run --release -- --export docs   → docs/img/*.png y docs/frames/<clip>/*.png
#   2. ffmpeg arma docs/gif/<clip>.gif con una paleta propia por clip (mejor calidad de color)
# Requiere ffmpeg. Uso: scripts/make_gifs.sh [ancho]   (480 px por defecto)
set -euo pipefail
cd "$(dirname "$0")/.."
WIDTH="${1:-480}"

cargo run --release -- --export docs
mkdir -p docs/gif
for dir in docs/frames/*/; do
    name="$(basename "$dir")"
    fps="$(cat "$dir/fps.txt")"
    filters="fps=$fps,scale=$WIDTH:-1:flags=lanczos"
    ffmpeg -loglevel error -y -framerate "$fps" -i "$dir/%04d.png" \
        -vf "$filters,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4" \
        "docs/gif/$name.gif"
    echo "docs/gif/$name.gif"
done
rm -rf docs/frames

# ImageCompress

A Python-based image compression tool with support for multiple formats and optimization strategies.

## Setup

This project uses [UV](https://github.com/astral-sh/uv) for dependency management with Python 3.12.

### Prerequisites
- UV package manager (install from https://github.com/astral-sh/uv)
- External compression tools installed via Homebrew:
  ```bash
  brew install pngquant oxipng mozjpeg jpeg-xl imagemagick cavif libheif gifsicle
  ```

### Installation
```bash
# UV will automatically create a virtual environment and install dependencies
uv sync
```

## Usage

Run the compression tool with:
```bash
uv run compress.py [options] <files or directories>
```

### Examples
```bash
# Compress images in a directory (lossless)
uv run compress.py ~/Pictures

# Compress with lossy optimization
uv run compress.py --lossy ~/Pictures

# Convert to JPEG format
uv run compress.py --jpg ~/Pictures

# Verbose output with specific parallelism
uv run compress.py -v --parallelism 2 image.png
```

### Options
- `-v`: Enable verbose output
- `-q`: Quiet mode (no output)
- `--lossy`: Enable lossy optimizations
- `--jpg`: Convert images to JPEG
- `--png`: Convert images to PNG
- `--heif`: Convert images to HEIF format
- `--avif`: Convert images to AVIF format
- `--jxl`: Convert images to JPEG-XL format
- `--deleteOnConvert`: Delete original after format conversion
- `--zopfli`: Use Zopfli compression for PNGs (slower but better compression)
- `--parallelism N`: Number of concurrent processes (default: 4)
- `--history FILE`: Track processed files to avoid recompressing

## Features

- Compress selected items (including recursion) with lossy or lossless optimization
- Convert between PNG, JPG, HEIF, AVIF, and JPEG-XL formats
- Lossless and lossy compression modes
- Parallel processing support
- Compression history tracking

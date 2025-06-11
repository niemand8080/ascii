# ASCII

For the Video to ASCII part it is recommended to use a very fast terminal like [alacritty](https://alacritty.org), if you want to ouput it to stdout.

## TODO

  - [ ] Make this (a lot) faster.
  - [ ] Make it a CLI tool.

## Requirements

1. FFmpeg (I only tested version 7.1.1)
2. pkg-config
3. The path where all .pc file of the FFmpeg libraries are located e.g. `/opt/homebrew/lib/pkgconfig`

## Installation

1. Clone the repo

```shell 
git clone https://github.com/niemand8080/ascii
```

```shell 
cd ascii
```

2. run something like the following (you may want to change the `src/main.rs` file)

```shell
PKG_CONFIG_PATH=:/opt/homebrew/lib/pkgconfig cargo run --release
```

## Examples

### Image to ASCII

Images if from here: [torii-gate-japan](https://static6.depositphotos.com/1128318/616/i/450/depositphotos_6161942-stock-photo-torii-gate-japan.jpg)

![ascii](examples/ascii-torii-gate-japan.jpg)
![original](examples/torii-gate-japan.jpg)

### Video to ASCII

Video is from here: [BigBuckBunny](http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4)

![ascii](https://github.com/user-attachments/assets/10330e68-1ac3-4bf8-ba5a-a027c6f9b8be)
![original](https://github.com/user-attachments/assets/60b04f96-a1ef-4783-b9e9-27d41a1616ca)

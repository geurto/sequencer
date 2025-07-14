# Generative Sequencer (needs a cooler name)
This is a collection of generative sequencers, written in Rust. Use it to send MIDI notes to your synthesizer or DAW.
Sequencers currently supported:
- Euclidean

## Installation on RaspBerry Pi
This is put here as reference, in case the original link (https://aidanblack.github.io/RPiMIDI.html) goes offline.

### Remove baud setting from boot command
`sudo nano /boot/firmware/cmdline.txt`

Remove `console=serial0,115200`

### Add kernel overlays to config.txt
`sudo nano /boot/firmware/config.txt`

Add:
```
enable_uart=1
dtoverlay=pi3-miniuart-bt
dtoverlay=midi-uart
```
### Disable serial console
`sudo raspi-config`

Interfacing options -> P6 Serial -> Disable login shell -> Enable serial port

### Reboot
### Install TTYMIDI
`sudo apt-get install libasound2-dev`

```
wget http://www.varal.org/ttymidi/ttymidi.tar.gz
tar -zxvf ttymidi.tar.gz
cd ttymidi/
make
sudo make install
```

`ttymidi -s /dev/ttyAMA0 -b 38400 &` Will run ttymidi in the background.
`ttymidi -s /dev/ttyAMA0 -b 38400 -v` Will run ttymidi in verbose mode (midi traffic displayed on screen).

## Use
To operate the sequencer, you can attach a keyboard to whatever device is running this code, e.g. a Raspberry Pi. 

### Common keyboard shortcuts
The common keyboard shortcuts are:
- `SPACE` to start/stop the sequencer
- `TAB` to switch between active sequencer

### Active sequencer
- `W` to increase the MIDI note by 1
- `S` to decrease the MIDI note by 1
- `A` to decrease the MIDI note by one octave
- `D` to increase the MIDI note by one octave

### Euclidean sequencer keyboard shortcuts
- `UP` to increase the number of steps by 1
- `DOWN` to decrease the number of steps by 1
- `LEFT` to decrease the number of pulses by 1
- `RIGHT` to increase the number of pulses by 1

### Mixer
- `R` to increase mixer ratio by 0.05
- `F` to decrease mixer ratio by 0.05

## Try it out with FluidSynth
An easy way (on Linux) to get a feel for this sequencer is to attach it to a FluidSynth instance.

### Get a SoundFont file
Next, you'll need to download a .sf2 SoundFont file, such as here: [Roland SC-88 SoundFont file](https://musical-artifacts.com/artifacts/538)

## Roadmap
- [ ] Add chords
- [ ] Add scales
- [ ] Add +AI sequencer 

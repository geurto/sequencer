# Generative Sequencer (needs a cooler name)
This is a collection of generative sequencers, written in Rust. Use it to send MIDI notes to your synthesizer or DAW.
Sequencers currently supported:
- Euclidean
- Markov Chain

## Use
To operate the sequencer, you can attach a keyboard to whatever device is running this code, e.g. a Raspberry Pi. 

### Common keyboard shortcuts
The common keyboard shortcuts are:
- `SPACE` to start/stop the sequencer

### Euclidean sequencer keyboard shortcuts
- `UP` to increase the number of steps by 1
- `DOWN` to decrease the number of steps by 1
- `LEFT` to decrease the number of pulses by 1
- `RIGHT` to increase the number of pulses by 1
- `W` to increase the MIDI note by 1
- `S` to decrease the MIDI note by 1
- `A` to decrease the MIDI note by one octave
- `D` to increase the MIDI note by one octave

### Markov Chain sequencer keyboard shortcuts

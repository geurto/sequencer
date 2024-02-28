# Use the latest Ubuntu image as the base
FROM ubuntu:latest

# Avoid prompts from apt
ENV DEBIAN_FRONTEND=noninteractive

# Update and install necessary packages
RUN apt-get update && apt-get install -y \
    fluidsynth \
    pulseaudio \
    && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /usr/local/bin

# Copy super-saw.sf2
COPY super-saw.sf2 /usr/local/bin/soundfonts/

# Expose the FluidSynth port (if you're planning to connect via network)
EXPOSE 9800

# Run FluidSynth with the downloaded SoundFont.
# This command uses the SoundFont and sets FluidSynth to listen to MIDI input on all network interfaces.
# Adjust the command flags as needed.
CMD ["fluidsynth", "-a", "pulseaudio", "-m", "alsa_seq", "-l", "-i", "/usr/local/bin/soundfonts/super-saw.sf2", "-s", "-o", "audio.driver=pulseaudio", "-o", "midi.autoconnect=1", "-o", "shell.port=9800"]

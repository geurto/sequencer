Make sure that Docker containers can connect to PulseAudio. 
Add the following lines to your /etc/pulse/default.pa file:

```
`# Allow Docker containers to connect with PulseAudio
load-module module-native-protocol-tcp auth-ip-acl=127.0.0.1;172.17.0.0/16 auth-anonymous=1
load-module module-esound-protocol-tcp
```


Then restart PulseAudio:

`pulseaudio -k && pulseaudio --start`

Or, because connecting to PulseAudio over a Docker container does not work so far, 
just use the following command in your terminal so at least the Rust code produces a sound:

`fluidsynth -a pulseaudio -m alsa_seq -l -i /usr/local/bin/soundfonts/super-saw.sf2 -s -o audio.driver=pulseaudio -o midi.autoconnect=1 -o shell.port=9800`
Live captioner using the Azure Speech to Text API


## Installation on raspberry pi
* Run `make deploy-pi`
* Make the following files:

`/etc/xdg/lxsession/LXDE-pi/autostart`:
```
@lxpanel --profile LXDE-pi
@pcmanfm --desktop --profile LXDE-pi
@xscreensaver -no-splash
@chromium-browser --start-fullscreen --temp-profile http://localhost
@/home/david/x-config
```

`/home/david/x-config`:
```bash
#!/bin/bash

xrandr -d :0 --output HDMI-1 --mode 1920x1080 --rate 50
```

The LXDE autostart config specifies programs which should start at the time
LXDE loads - note that they are started roughly concurrently, and no
particular ordering can be assumed.

The `--temp-profile` option for chromium causes the browesr to skip its
"restore session" checks, which otherwise interfere with correct loading
of the application after a reboot.

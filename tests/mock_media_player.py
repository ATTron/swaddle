#!/usr/bin/env python3
"""
Mock MPRIS media player for testing swaddle D-Bus integration.
This creates a fake media player that reports "Playing" status.
"""

import os
import sys
import signal
import time
from threading import Thread

# Try to import D-Bus modules
try:
    import dbus
    import dbus.service
    import dbus.mainloop.glib
    from gi.repository import GLib
except ImportError:
    print("D-Bus modules not available, exiting")
    sys.exit(0)

# Set up D-Bus main loop
dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)


class MockMediaPlayer(dbus.service.Object):
    """Mock MPRIS MediaPlayer2.Player implementation."""

    def __init__(self, bus_name, object_path):
        super().__init__(bus_name, object_path)
        self.properties = {
            'org.mpris.MediaPlayer2.Player': {
                'PlaybackStatus': 'Playing',
                'CanPlay': True,
                'CanPause': True,
                'CanStop': True,
            }
        }

    @dbus.service.method('org.freedesktop.DBus.Properties', 
                        in_signature='ss', out_signature='v')
    def Get(self, interface_name, property_name):
        """Get a property value."""
        if interface_name in self.properties:
            return self.properties[interface_name].get(property_name, '')
        return ''

    @dbus.service.method('org.freedesktop.DBus.Properties',
                        in_signature='s', out_signature='a{sv}')
    def GetAll(self, interface_name):
        """Get all properties for an interface."""
        return self.properties.get(interface_name, {})

    @dbus.service.signal('org.freedesktop.DBus.Properties',
                        signature='sa{sv}as')
    def PropertiesChanged(self, interface_name, changed_properties, invalidated_properties):
        """Signal for property changes."""
        pass


def timeout_handler():
    """Exit after timeout to prevent hanging tests."""
    time.sleep(5)
    print("Mock player timeout reached, exiting")
    os._exit(0)


def signal_handler(signum, frame):
    """Handle termination signals gracefully."""
    print(f"Received signal {signum}, exiting gracefully")
    sys.exit(0)


def main():
    """Main function to run the mock media player."""
    signal.signal(signal.SIGTERM, signal_handler)
    signal.signal(signal.SIGINT, signal_handler)

    try:
        timeout_thread = Thread(target=timeout_handler, daemon=True)
        timeout_thread.start()

        bus = dbus.SessionBus()
        name = dbus.service.BusName('org.mpris.MediaPlayer2.mocktestplayer', bus)
        player = MockMediaPlayer(name, '/org/mpris/MediaPlayer2')

        print("Mock player registered: org.mpris.MediaPlayer2.mocktestplayer")
        print("PlaybackStatus: Playing")
        sys.stdout.flush()

        loop = GLib.MainLoop()
        loop.run()
    except Exception as e:
        print(f"Mock player failed: {e}")
        sys.exit(1)


if __name__ == '__main__':
    main()

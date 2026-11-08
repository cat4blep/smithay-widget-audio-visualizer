# Audio visualizer for Smithay Desktop

Shows the frequencies of audio playing on the default output. The panel displays 12 bars. The desktop card displays 24 bars and can be moved or resized in desktop edit mode.

## Install

Open **Widgets** in Smithay Desktop and paste `https://github.com/cat4blep/smithay-widget-audio-visualizer` into the GitHub field. Choose **Install**, then enable **Panel**, **Desktop**, or both.

The widget uses the Smithay Widget API v1. The shell reads the output monitor through PulseAudio or PipeWire with PulseAudio compatibility and `parec`. The bars follow the active Smithay palette. Audio samples are analyzed in memory while the visualizer is enabled.

To make a variant, copy [smithay-widget.toml](smithay-widget.toml) into the root of your repository, choose a new `id`, and adjust the number of bars in either view. Each view supports 4–32 bars.

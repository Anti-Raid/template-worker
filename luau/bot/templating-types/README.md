# templating-types
AntiRaid templating types

Discord typings here are provided by https://github.com/DiscordLuau/discord-luau (`refactor/monorepository-origin` branch) and as such may may be *newer* or *older* than AntiRaid provided data.

- ``@antiraid`` - AntiRaid core libs
- ``@antiraid-core`` - Primitives and other core things that exist in AntiRaid but are not part of the main library. **Note that these functions are not part of AntiRaid and must be bundled in using ``darklua`` etc**
- ``@antiraid-ext`` - Extra helper functions and library functions to make development easier. **Note that these functions are not part of AntiRaid and must be bundled in using ``darklua`` etc**

## Bundling

A bundler which supports ``luaurc`` path aliases is required to use ``templating-types``. Currently darklua using the ``luaurc-support`` PR is the best option we've found for this. Use ``setup_dev.sh`` for building this.
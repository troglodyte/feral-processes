feral-processes
===============

A headless ECS game sim with a graphical renderer. Run
feral-processes.exe to play.


Where your saves go
-------------------

%APPDATA%\feral-processes\

Saves, your profile (which holds the achievement ladder's earned
rewards) and the run history all live there, not in this folder. That
means you can move, replace or delete this folder without losing
anything, and an update is just an unzip over the top.


Mods
----

Game content is data, not code. Every species, item, structure,
ability, talent tree, perk, achievement and help page in the game is a
file under assets\, and adding one is dropping a file in — no rebuild.
Each assets\<thing>\ directory has a README.md documenting its schema.

A malformed file is skipped with a warning rather than crashing the
game, so a mod that does not parse costs you that one file and nothing
else.


Windows will warn you on first run
----------------------------------

This build is not code-signed, so SmartScreen shows a blue "Windows
protected your PC" box. Click "More info", then "Run anyway".

That warning means the build has no certificate, not that anything is
wrong with it. Signing requires buying a certificate, which this
project has not done.


If it will not start
--------------------

Look for startup-error.txt beside the executable. The two things that
stop the game before it opens a window are a missing assets\ folder
(unzip the whole archive, not just the .exe) and a %APPDATA% directory
it cannot create.

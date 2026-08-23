feral-processes
===============

A headless ECS game sim with a graphical renderer.

Open Terminal, cd into this folder, and run:

    ./feral-processes

This is a plain executable rather than a .app bundle, so
double-clicking it in Finder opens a Terminal window and runs it there.
That works, but the Terminal window stays open behind the game.


macOS will refuse to run it the first time
------------------------------------------

This build is not signed or notarized, and anything downloaded through a
browser is quarantined. Gatekeeper will say the developer cannot be
verified. Clear the quarantine flag on the whole folder:

    xattr -dr com.apple.quarantine .

Then run it again. You only need to do this once per download.

The alternative, if you would rather not run that: try to launch it,
then go to System Settings -> Privacy & Security, find the blocked item
near the bottom, and click "Open Anyway".

Neither warning means anything is wrong with the build. Signing and
notarizing require a paid Apple Developer account, which this project
does not have.


Where your saves go
-------------------

~/Library/Application Support/feral-processes/

Saves, your profile (which holds the achievement ladder's earned
rewards) and the run history all live there, not in this folder. That
means you can move, replace or delete this folder without losing
anything, and an update is just an unzip over the top.

Finder hides ~/Library by default. Go -> Go to Folder, then paste the
path.


Mods
----

Game content is data, not code. Every species, item, structure,
ability, talent tree, perk, achievement and help page in the game is a
file under assets/, and adding one is dropping a file in — no rebuild.
Each assets/<thing>/ directory has a README.md documenting its schema.

A malformed file is skipped with a warning rather than crashing the
game, so a mod that does not parse costs you that one file and nothing
else.


If it will not start
--------------------

Look for startup-error.txt beside the executable. The two things that
stop the game before it opens a window are a missing assets/ folder
(unzip the whole archive, not just the binary) and a data directory it
cannot create.

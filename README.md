# ZeroSplitter
Automatic split-tracker for ZeroRanger. Very beta. 
Supports Green Orange and White Vanilla.

[DOWNLOAD](https://github.com/lily-and-doll/ZeroSplitter/releases)

<img width="302" height="677" alt="zerosplitter_hjuYG7gioU" src="https://github.com/user-attachments/assets/77ad0430-7060-4035-9c88-a3d741ab65a6" />
<img width="302" height="677" alt="zerosplitter_1v8gZHNMn1" src="https://github.com/user-attachments/assets/268c1539-83bb-4c07-bc0a-31cf4515ac67" />


# How to use
Extract the zip and run `zerosplitter.exe`. The program will detect Zeroranger and start reading data automatically.
You can start playing and your scores will automatically show up in ZeroSplitter. If you restart your run and your score
doesn't show up as the correct split, just go back to the main menu and launch from there.

Continues should be tracked properly in Green Orange but not White Vanilla.

Your data is stored in the `zs_data.json` next to the .exe.

A "category" is a set of splits and personal bests to run against. ZeroSplitter will try to detect which mode
you are playing and not overwrite scores from one mode with another - but don't push your luck: have the right 
category selected before you take off.

Press the plus button to add a new category - don't click Black Onion for the mode (it won't work at all). 

Currently the only way to delete categories is by manually deleting them in the `zs_data.json`, but you can rename them.

The "relative" button switches the display between showing your score per split or your running total up to each split.
Turn on relative mode to see how much better or worse you did each split versus your PB run. Turn off relative mode
to see how far ahead or behind you are versus your PB run.

The relative button only changes the display, not how the data is saved: toggle it as much as you like, even mid-run.

If you want to move the program to another folder, just copy `zerosplitter.exe`, `payload.dll`, and `zs_data.json`.

# How to build from source
Just run `cargo run --release` in the top level of the repository, next to this `README.md`. `build.sh` will zip `zerosplitter.exe` 
and `payload.dll` for you, but you don't need to do this.

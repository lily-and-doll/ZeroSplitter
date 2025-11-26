# ZeroSplitter
Automatic split-tracker for ZeroRanger. Very beta. 
Supports Green Orange and White Vanilla.

[DOWNLOAD](https://github.com/lily-and-doll/ZeroSplitter/releases)

<img width="302" height="682" alt="zerosplitter_BjsRefSLvF" src="https://github.com/user-attachments/assets/9ad379b2-5a8c-48bb-ae3f-423f39c98661" />
<img width="302" height="682" alt="zerosplitter_vVeE4jqNXy" src="https://github.com/user-attachments/assets/b0e8cc7a-2ef4-4f2b-b42c-296c3f124acf" />


# How to use
Extract the zip and run `zerosplitter.exe`. The program will detect Zeroranger and start reading data automatically.
You can start playing and your scores will automatically show up in ZeroSplitter. If you restart your run and your score
doesn't show up as the correct split, just go back to the main menu and launch from there.

Continues should be tracked properly in Green Orange but not White Vanilla.

Co-op should work, but has not been tested. If you find co-op to work, let me know.

# Your Data
Your data is stored in the `sqlite.db3` next to the .exe. You can manually insert, delete, or modify any data in the database if you like.
When updating the program, just put the new `zerosplitter.exe` and `payload.dll` in the same folder as your old `sqlite.db3` and `config.toml` files.
The database file will automatically be updated and will not be able to be used with older versions of the program.

If you want to move the program to another folder, just copy all the files in the folder. 

# Categories
A "category" is a set of splits and personal bests to run against. ZeroSplitter will try to detect which mode
you are playing and not overwrite scores from one mode with another - but don't push your luck: have the right 
category selected before you take off.

Press the plus button to add a new category.

Currently the only way to delete categories is by manually dropping them from the database, but you can rename them.

# Toggles
The "relative" button switches the display between showing your score per split or your running total up to each split.
Turn on relative mode to see how much better or worse you did each split versus your PB run. Turn off relative mode
to see how far ahead or behind you are versus your PB run.

The relative button only changes the display, not how the data is saved: toggle it as much as you like, even mid-run.

The Best Splits button switches the left-hand score display between your best score for a split or the splits of your PB.

The Names button switches the split names between numbers (1-2) and names (Cloudoos). White Vanilla only.

# Options
The gear button in the top right opens up the options menu. 
You can import previously recorded runs by putting a category name and a list of scores into the two boxes.
Seperate each score with a comma and space like shown in the hint.

# How to build from source
Just run `cargo run --release` in the top level of the repository, next to this `README.md`. `build.sh` will zip `zerosplitter.exe` 
and `payload.dll` for you, but you don't need to do this.

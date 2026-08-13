# Todos
1. companions in the 'party' list should show 'w|a|m' for equipped slots
2. some ambiguity|routines for examining entities
3. environment effects
4. unorthodox solutions
5. high stakes reward
6. easter eggs (hidden commands & effects)
7. remove 'move abilities' and use routines
8. more machine learning
9. i want the description of what i'm seeing as i'm walking in the stack, not with x | examine
10. when a stack boss is defeated, the stack should collapse, and the player gets kicked out, and the entrance is deleted (no need to keep in memory), new stack appears somewhere nearby
11. structures to keep base
12. next zone unlocks are research -> upgrade base to zone 2
13. one button to fuse all items

# Bugs
1. A maxed-fusion Gold affixed copy overflows the inventory popup: the widest
   the shipped assets can build is 1311px into a 1243px body at zone 10, and
   the row runs off the right edge taking its equip tag with it. The excess is
   `equip_preview_tag`'s `" - maxed"`, appended on the stated grounds that
   this screen has the room. Pre-existing, measured 2026-08-13;
   `no_shipped_inventory_row_overflows_its_popup` covers every tier below
   maxed and excludes that case by name.

# The ground

- **`Game::ground_effect` is the one door onto what terrain does to you, and
  the zone-1 gate lives inside it.** A second caller resolving a biome
  against `EnvironmentDb` itself is how neutral zone 1 lapses. **The trap is
  that the biome's *name* is deliberately outside that gate** — `Biome::name`
  and the crossing line in `move_player` fire from the first step of a run,
  and `zone_one_takes_no_bite_but_still_names_the_ground` asserts both halves
  in one test because the effect half alone passes against a bare early
  return that swallowed the name too. Terrain never costs Power and never
  raises Trace, the player alone takes the damage, and the bite goes through
  `Game::apply_damage` — which is the whole of why mitigation applies to
  ground for free. Deleting `assets/environment/` restores the pre-effects
  game exactly, the same supported way deleting `assets/sectors/` does.

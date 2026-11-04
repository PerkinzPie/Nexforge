# CurseForge fixtures (synthetic)

These are **not recordings**. No CurseForge API key was available when the
source was written (2026-11-04), so the responses were built by hand from the
schemas in docs.curseforge.com (`Mod`, `File`, `FileIndex`, `HashAlgo`,
`FileReleaseType`, pagination). Real names: DungeonJournal by ExehnTV is
CurseForge project 1707971 for game version 1.60.1. Everything else (file ids,
versions, hashes, image URLs on `media.forgecdn.net`, project 200000) is made up.

`PLACEHOLDER_MD5` is replaced at test time with the MD5 of the synthetic zip.

Replace these with recorded responses (trimmed, with the key removed) as soon
as a key is available, and confirm the image host in `net.rs`.

import { describe, expect, it } from "vitest";
import { ago, count, flavourTile, monogram, plural, size, tint } from "../format";

describe("format", () => {
  it("relative times", () => {
    const now = Date.parse("2026-11-04T12:00:00Z");
    expect(ago("2026-11-04T11:59:40Z", now)).toBe("just now");
    expect(ago("2026-11-04T11:58:00Z", now)).toBe("2 min ago");
    expect(ago("2026-11-04T09:00:00Z", now)).toBe("3 h ago");
    expect(ago("2026-11-03T12:00:00Z", now)).toBe("yesterday");
    expect(ago(null, now)).toBe("");
  });

  it("sizes and counts", () => {
    expect(size(2_200_000)).toBe("2.1 MB");
    expect(size(1_411 * 1024 * 1024)).toBe("1.4 GB");
    expect(size(800)).toBe("1 KB");
    expect(size(null)).toBe("");
    expect(count(1_700_000)).toBe("1.7M");
    expect(count(2_000_000)).toBe("2M");
    expect(count(840_000)).toBe("840K");
    expect(plural(1, "addon")).toBe("1 addon");
    expect(plural(3, "addon")).toBe("3 addons");
  });

  it("tiles are stable per name", () => {
    expect(tint("Questie")).toEqual(tint("Questie"));
    expect(monogram("!Details")).toBe("D");
    expect(monogram("???")).toBe("?");
    expect(flavourTile("forever").letter).toBe("F");
    expect(flavourTile("classic-era").letter).toBe("E");
  });
});

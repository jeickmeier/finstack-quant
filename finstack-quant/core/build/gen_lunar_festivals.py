"""Generate Gregorian dates for Chinese festivals and Japanese equinoxes.

Dragon Boat Festival (端午节) = 5th day of the 5th lunar month.
Mid-Autumn Festival (中秋节)  = 15th day of the 8th lunar month.
Buddha's Birthday (佛誕) = 8th day of the 4th lunar month.
Chung Yeung Festival (重陽節) = 9th day of the 9th lunar month.
Ching Ming uses the solar term in UTC+8; Japanese equinoxes use UTC+9.

Dragon Boat and Mid-Autumn became national statutory holidays in mainland China
in 2008; the tables are generated for the full calendar range (1970-2150) for uniformity with
``chinese_new_year.csv`` / ``cny_generated.rs``. Per-calendar year gating (e.g.
``from_year: 2008``) is applied in the JSON calendar definitions, not here.

Source: the ``lunar-python`` package (6tail lunar library, calibrated to the
official 紫金山天文台 calendar), which agrees with ``lunardate`` across 1970-2099
and extends to 2150.

Regenerate with:

    uv run --with lunar_python finstack-quant/core/build/gen_lunar_festivals.py

Outputs (relative to the crate root ``finstack-quant/core``):
  - data/dragon_boat.csv
  - data/mid_autumn.csv
  - data/buddhas_birthday.csv
  - data/chung_yeung.csv
  - data/qing_ming.csv
  - data/vernal_equinox_jp.csv
  - data/autumnal_equinox_jp.csv
  - src/generated/festivals_generated.rs
"""

from __future__ import annotations

from datetime import datetime, timedelta
from pathlib import Path
import sys

from generate_reference_data import render_festivals
from lunar_python import Lunar, Solar

BASE_YEAR = 1970
END_YEAR = 2150

# (lunar_month, lunar_day) for each festival.
DRAGON_BOAT = (5, 5)
MID_AUTUMN = (8, 15)
BUDDHAS_BIRTHDAY = (4, 8)
CHUNG_YEUNG = (9, 9)


def festival_dates(lunar_month: int, lunar_day: int) -> list[tuple[int, int, int]]:
    """Resolve a lunar festival to Gregorian dates over the supported year range."""
    rows: list[tuple[int, int, int]] = []
    for year in range(BASE_YEAR, END_YEAR + 1):
        solar = Lunar.fromYmd(year, lunar_month, lunar_day).getSolar()
        sy, sm, sd = solar.getYear(), solar.getMonth(), solar.getDay()
        if sy != year:
            raise ValueError(f"festival {lunar_month}/{lunar_day} for {year} fell in {sy}")
        rows.append((year, sm, sd))
    return rows


def write_csv(path: Path, rows: list[tuple[int, int, int]]) -> None:
    """Write year, month, and day observations in deterministic order."""
    lines = ["year,month,day"]
    lines += [f"{y},{m},{d}" for (y, m, d) in rows]
    path.write_text("\n".join(lines) + "\n")


def solar_term_dates(name: str, utc8_offset: int) -> list[tuple[int, int, int]]:
    """Resolve a solar term, converting the library's UTC+8 time to local time.

    Hong Kong uses UTC+8; Japanese equinox holidays use UTC+9. Astronomical
    projections for future Japanese holidays remain subject to official annual
    announcement by the National Astronomical Observatory of Japan.
    """
    rows = []
    for year in range(BASE_YEAR, END_YEAR + 1):
        solar = Solar.fromYmd(year, 4, 1).getLunar().getJieQiTable()[name]
        instant = datetime(
            solar.getYear(),
            solar.getMonth(),
            solar.getDay(),
            solar.getHour(),
            solar.getMinute(),
            solar.getSecond(),
        ) + timedelta(hours=utc8_offset)
        rows.append((year, instant.month, instant.day))
    return rows


def main() -> None:
    """Refresh festival/equinox CSV inputs and corresponding Rust tables."""
    crate_root = Path(__file__).resolve().parents[1]
    data_dir = crate_root / "data"
    gen_dir = crate_root / "src" / "generated"

    tables = {
        "dragon_boat": festival_dates(*DRAGON_BOAT),
        "mid_autumn": festival_dates(*MID_AUTUMN),
        "buddhas_birthday": festival_dates(*BUDDHAS_BIRTHDAY),
        "chung_yeung": festival_dates(*CHUNG_YEUNG),
        "qing_ming": solar_term_dates("清明", 0),
        "vernal_equinox_jp": solar_term_dates("春分", 1),
        "autumnal_equinox_jp": solar_term_dates("秋分", 1),
    }
    for name, rows in tables.items():
        write_csv(data_dir / f"{name}.csv", rows)
    (gen_dir / "festivals_generated.rs").write_text(render_festivals(tables))
    sys.stdout.write(f"Wrote seven festival/equinox tables for {BASE_YEAR}-{END_YEAR}.\n")


if __name__ == "__main__":
    main()

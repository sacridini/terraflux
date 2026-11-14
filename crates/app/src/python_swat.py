# Terraflux's SWAT+ operators in Python: the tables of the weather and of the results as Terraflux gives them, and
# swat.set's change (a row of calibration.cal, and of cal_parms.cal when the project lacks the parameter).
import pandas as _pd


def _swat_dates(t):
    """The dates of a table of days: a column `date` (2001-01-31 or 20010131), else `year` and `day` (of the year)."""
    if "date" in t.columns:
        return _pd.to_datetime(t["date"].astype(str).str.strip().str[:10], format="mixed")
    year = _pd.to_datetime(t["year"].astype(int).astype(str), format="%Y")
    return year + _pd.to_timedelta(t["day"].astype(int) - 1, unit="D")


def _swat_series(t):
    """A daily series of the weather as landschaft takes it: the days as the index, a column per station."""
    t = t.copy()
    days = _swat_dates(t)
    t = t.drop(columns=[c for c in ("date", "year", "day") if c in t.columns])
    t.index = _pd.DatetimeIndex(days)
    return t.sort_index()


def _swat_stations(t):
    """The weather stations as landschaft takes them: named by the index."""
    return t.set_index("name" if "name" in t.columns else t.columns[0])


def _swat_wgn(t):
    """The weather generator's statistics as landschaft takes them: indexed by station and month."""
    return t.set_index(["station", "month"]) if "station" in t.columns else t


def _swat_wgn_table(t):
    """The statistics as a table: the station and the month first."""
    return t.reset_index()


def _swat_table(result, name, step):
    """A table SWAT+ printed, as Terraflux gives it: the year and the day of the year, the object, the columns."""
    t = result.table(name, step).reset_index()
    days = _pd.to_datetime(t.pop("date"))
    t.insert(0, "year", days.dt.year)
    t.insert(1, "day", days.dt.dayofyear)
    return t


def _swat_weather(result):
    """The daily weather of each station SWAT+ used, as Terraflux gives it."""
    t = result.weather.reset_index()
    days = _pd.to_datetime(t.pop("date"))
    t.insert(0, "year", days.dt.year)
    t.insert(1, "day", days.dt.dayofyear)
    return t


def _swat_discharge(q):
    """The discharge by date and channel (landschaft's reach_discharge) as Terraflux gives it: year, day, channel_N."""
    t = q.copy()
    t.columns = [f"channel_{c}" for c in t.columns]
    t = t.reset_index()
    days = _pd.to_datetime(t.pop(t.columns[0]))
    t.insert(0, "year", days.dt.year)
    t.insert(1, "day", days.dt.dayofyear)
    return t


def _swat_values(t):
    """Values by object (HRU, channel): a table of the object's number and its value, as a series by number."""
    return _pd.Series(t.iloc[:, 1].to_numpy(dtype=float), index=t.iloc[:, 0].astype(int).to_numpy())


def _swat_number(x):
    """A number as Terraflux writes it in a file (10, not 10.0)."""
    x = float(x)
    return str(int(x)) if x.is_integer() and abs(x) < 1e15 else repr(x)


def _swat_nyskip(project, nyskip):
    """A copy of the project that skips `nyskip` years before it prints (what a map's own run does)."""
    project = project.copy()
    project.set_print(nyskip=int(nyskip))
    return project


def _swat_cio(project, line, k, default):
    """The `k`th file of a line of file.cio (named in file.cio, with `default`, when it was not)."""
    lines = project.text("file.cio").split("\n")
    at = next(i for i, ln in enumerate(lines) if ln.split()[:1] == [line])
    words = lines[at].split()
    if len(words) > k + 1 and words[k + 1] != "null":
        return words[k + 1]
    if len(words) <= k + 1:
        words += ["null"] * (k + 2 - len(words))
    words[k + 1] = default
    lines[at] = "".join(f"{w:<17} " for w in words)
    project.set_text("file.cio", "\n".join(lines))
    return default


def _swat_add_row(project, name, title, header, row):
    """A row added to a file of a count and a header (cal_parms.cal, calibration.cal): after the counted rows."""
    text = project.text(name) if name in project else f"{name}: {title}\n0\n{header}\n"
    lines = text.split("\n")
    while lines and not lines[-1].strip():
        lines.pop()
    try:
        n = int(lines[1].strip())
    except (IndexError, ValueError):
        n = 0
    lines.insert(min(3 + n, len(lines)), row)
    lines[1] = str(n + 1)
    project.set_text(name, "\n".join(lines))


def _swat_set(project, parameter, change="absval", value=0.0, first_layer=0, last_layer=0, objects=()):
    """swat.set: a copy of the project with a parameter changed as SWAT+'s hard calibration does (a row of
    calibration.cal; the parameter's of cal_parms.cal when the project lacks it, with the port's default limits)."""
    if parameter not in _SWAT_PARAMETERS:
        raise ValueError(f"the parameter {parameter!r} is not one the port calibrates")
    kind, low, high = _SWAT_PARAMETERS[parameter]
    if change not in ("pctchg", "abschg", "absval"):
        raise ValueError(f"change must be pctchg, abschg or absval, got {change!r}")
    objects = [int(o) for o in objects or ()]
    project = project.copy()
    parms = _swat_cio(project, "chg", 0, "cal_parms.cal")
    known = parms in project and any(ln.split()[:1] == [parameter] for ln in project.text(parms).split("\n")[3:])
    if not known:
        row = f"{parameter:<30}{kind:>10}{low:>14.5f}{high:>14.5f}{'null':>18}  "
        _swat_add_row(project, parms, "written by Terraflux",
                      "name                       obj_typ       abs_min       abs_max             units  ", row)
    upd = _swat_cio(project, "chg", 1, "calibration.cal")
    row = (f"{parameter:<24}{change:>10}{_swat_number(value):>14}{0:>8}{int(first_layer):>10}{int(last_layer):>10}"
           f"{0:>10}{0:>10}{0:>10}{0:>10}{len(objects):>10}" + "".join(f"{o:>8}" for o in objects))
    _swat_add_row(project, upd, "written by Terraflux",
                  "cal_parm               chg_typ       chg_val     conds  soil_lyr1  soil_lyr2       yr1       yr2"
                  "      day1      day2   obj_tot", row)
    return project

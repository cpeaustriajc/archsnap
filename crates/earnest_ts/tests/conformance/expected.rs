use earnest_check::FindingKind;

pub(crate) enum Shortest {
    Steps(&'static [&'static str]),
    Length(usize),
}

pub(crate) struct Expected {
    pub(crate) file: &'static str,
    pub(crate) model: &'static str,
    pub(crate) states: usize,
    pub(crate) holds: &'static [&'static str],
    pub(crate) unknown: &'static [&'static str],
    pub(crate) broken: &'static [(&'static str, Shortest)],
    pub(crate) starts_with: &'static [(&'static str, &'static str)],
    pub(crate) findings: &'static [(FindingKind, &'static [&'static str])],
}

const fn passing(
    file: &'static str,
    model: &'static str,
    states: usize,
    holds: &'static [&'static str],
) -> Expected {
    Expected {
        file,
        model,
        states,
        holds,
        unknown: &[],
        broken: &[],
        starts_with: &[],
        findings: &[],
    }
}

pub(crate) const MODELS: &[Expected] = &[
    Expected {
        findings: &[(FindingKind::Fault, &["clear"])],
        unknown: &["capped"],
        ..passing("basket.model.ts", "Basket", 4, &[])
    },
    Expected {
        broken: &[("focusLands", Shortest::Steps(&["press", "land"]))],
        starts_with: &[("surface", "\"shortlist\"")],
        ..passing("cart.model.ts", "Cart", 6, &[])
    },
    passing("counter.model.ts", "Counter", 4, &["bounded", "even"]),
    Expected {
        broken: &[("near", Shortest::Length(10))],
        ..passing("dial.model.ts", "Dial", 36, &[])
    },
    Expected {
        findings: &[(FindingKind::Stuck, &["start", "fail"])],
        ..passing("download.model.ts", "Download", 4, &["whole"])
    },
    passing("grid.model.ts", "Grid", 27_000, &["inside"]),
    passing("heater.model.ts", "Heater", 8, &["capped"]),
    Expected {
        broken: &[("dim", Shortest::Steps(&["brighten", "brighten", "toggle"]))],
        starts_with: &[("power", "true")],
        ..passing("lamp.model.ts", "Lamp", 6, &[])
    },
    Expected {
        broken: &[(
            "bills",
            Shortest::Steps(&["deliver[letter = \"Bill\"]", "deliver[letter = \"Bill\"]"]),
        )],
        ..passing("mailbox.model.ts", "Mailbox", 7, &["fronted"])
    },
    Expected {
        findings: &[(FindingKind::Fault, &["drain", "drain"])],
        unknown: &["floor"],
        ..passing("meter.model.ts", "Meter", 3, &[])
    },
    Expected {
        broken: &[
            ("safe", Shortest::Steps(&["start", "open"])),
            ("closed", Shortest::Steps(&["open"])),
        ],
        ..passing("oven.model.ts", "Oven", 4, &[])
    },
    Expected {
        broken: &[(
            "alone",
            Shortest::Steps(&["arrive[guest = \"Ann\"]", "arrive[guest = \"Bob\"]"]),
        )],
        ..passing(
            "party.model.ts",
            "Party",
            4,
            &["algebra", "members", "counted"],
        )
    },
    passing("printer.model.ts", "Printer", 5, &["guarded", "owned"]),
    Expected {
        broken: &[(
            "single",
            Shortest::Steps(&["stock[item = \"Pen\"]", "restock[item = \"Pen\"]"]),
        )],
        ..passing("stock.model.ts", "Stock", 6, &["counted", "known"])
    },
    passing("switches.model.ts", "Porch", 2, &["porchKnown"]),
    passing("switches.model.ts", "Hall", 3, &["hallKnown"]),
    Expected {
        findings: &[(FindingKind::OutOfRange, &["add", "add", "add", "add"])],
        unknown: &["bounded"],
        ..passing("tally.model.ts", "Tally", 5, &[])
    },
    passing("ticket.model.ts", "Ticket", 12, &["seated"]),
    Expected {
        broken: &[(
            "ordered",
            Shortest::Steps(&["visit[stop = \"South\"]", "visit[stop = \"North\"]"]),
        )],
        ..passing("trail.model.ts", "Trail", 7, &["ends", "kept"])
    },
    Expected {
        broken: &[("locked", Shortest::Steps(&["push"]))],
        ..passing("turnstile.model.ts", "Turnstile", 4, &[])
    },
    passing(
        "upload.model.ts",
        "Upload",
        16,
        &["quiet", "waiting", "counted"],
    ),
];

pub(crate) const TANK_BEFORE: Expected = Expected {
    broken: &[("capped", Shortest::Steps(&["fill", "fill", "fill"]))],
    ..passing(
        "changes/before/tank.model.ts",
        "Tank",
        4,
        &["floor", "steady", "named", "gone", "kept"],
    )
};

pub(crate) const TANK_AFTER: Expected = Expected {
    broken: &[("floor", Shortest::Steps(&[]))],
    ..passing(
        "changes/after/tank.model.ts",
        "Tank",
        4,
        &["capped", "steady", "named", "kept", "fresh"],
    )
};

pub(crate) const TANK_BEFORE_RULES: &[(&str, &str, &str)] = &[
    ("capped", "The level is at most two", "s.level <= 2"),
    ("floor", "The level is at least zero", "s.level >= 0"),
    ("steady", "The level stays within the tank", "s.level <= 3"),
    ("named", "The level is small", "s.level < 4"),
    ("gone", "The level is never five", "s.level !== 5"),
    ("kept", "The level is never six", "s.level !== 6"),
];

pub(crate) const TANK_AFTER_RULES: &[(&str, &str, &str)] = &[
    ("capped", "The level is at most three", "s.level <= 3"),
    ("floor", "The level is at least one", "s.level >= 1"),
    ("steady", "The level stays within the tank", "s.level < 4"),
    ("named", "The level is tiny", "s.level < 4"),
    ("kept", "The level is never six", "s.level !== 6"),
    ("fresh", "The level is never seven", "s.level !== 7"),
];

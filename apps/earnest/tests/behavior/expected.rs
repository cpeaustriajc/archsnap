pub(crate) enum Shortest {
    Steps(&'static [&'static str]),
    Length(usize),
}

pub(crate) struct Expected {
    pub(crate) file: &'static str,
    pub(crate) model: &'static str,
    pub(crate) status: &'static str,
    pub(crate) states: u64,
    pub(crate) holds: &'static [&'static str],
    pub(crate) broken: &'static [(&'static str, Shortest)],
    pub(crate) findings: &'static [(&'static str, Shortest)],
}

pub(crate) const MODELS: &[Expected] = &[
    Expected {
        file: "basket.model.ts",
        model: "Basket",
        status: "fail",
        states: 4,
        holds: &["capped"],
        broken: &[],
        findings: &[("fault", Shortest::Steps(&["clear"]))],
    },
    Expected {
        file: "cart.model.ts",
        model: "Cart",
        status: "fail",
        states: 6,
        holds: &[],
        broken: &[("focusLands", Shortest::Steps(&["press", "land"]))],
        findings: &[],
    },
    Expected {
        file: "counter.model.ts",
        model: "Counter",
        status: "pass",
        states: 4,
        holds: &["bounded", "even"],
        broken: &[],
        findings: &[],
    },
    Expected {
        file: "dial.model.ts",
        model: "Dial",
        status: "fail",
        states: 36,
        holds: &[],
        broken: &[("near", Shortest::Length(10))],
        findings: &[],
    },
    Expected {
        file: "download.model.ts",
        model: "Download",
        status: "fail",
        states: 4,
        holds: &["whole"],
        broken: &[],
        findings: &[("stuck", Shortest::Steps(&["start", "fail"]))],
    },
    Expected {
        file: "grid.model.ts",
        model: "Grid",
        status: "pass",
        states: 27_000,
        holds: &["inside"],
        broken: &[],
        findings: &[],
    },
    Expected {
        file: "heater.model.ts",
        model: "Heater",
        status: "pass",
        states: 8,
        holds: &["capped"],
        broken: &[],
        findings: &[],
    },
    Expected {
        file: "lamp.model.ts",
        model: "Lamp",
        status: "fail",
        states: 6,
        holds: &[],
        broken: &[("dim", Shortest::Steps(&["brighten", "brighten", "toggle"]))],
        findings: &[],
    },
    Expected {
        file: "mailbox.model.ts",
        model: "Mailbox",
        status: "fail",
        states: 7,
        holds: &["fronted"],
        broken: &[("bills", Shortest::Steps(&["deliver", "deliver"]))],
        findings: &[],
    },
    Expected {
        file: "meter.model.ts",
        model: "Meter",
        status: "fail",
        states: 3,
        holds: &["floor"],
        broken: &[],
        findings: &[("fault", Shortest::Steps(&["drain", "drain"]))],
    },
    Expected {
        file: "oven.model.ts",
        model: "Oven",
        status: "fail",
        states: 4,
        holds: &[],
        broken: &[
            ("safe", Shortest::Steps(&["start", "open"])),
            ("closed", Shortest::Steps(&["open"])),
        ],
        findings: &[],
    },
    Expected {
        file: "party.model.ts",
        model: "Party",
        status: "fail",
        states: 4,
        holds: &["algebra", "members", "counted"],
        broken: &[("alone", Shortest::Steps(&["arrive", "arrive"]))],
        findings: &[],
    },
    Expected {
        file: "printer.model.ts",
        model: "Printer",
        status: "pass",
        states: 5,
        holds: &["guarded", "owned"],
        broken: &[],
        findings: &[],
    },
    Expected {
        file: "stock.model.ts",
        model: "Stock",
        status: "fail",
        states: 6,
        holds: &["counted", "known"],
        broken: &[("single", Shortest::Steps(&["stock", "restock"]))],
        findings: &[],
    },
    Expected {
        file: "switches.model.ts",
        model: "Porch",
        status: "pass",
        states: 2,
        holds: &["porchKnown"],
        broken: &[],
        findings: &[],
    },
    Expected {
        file: "switches.model.ts",
        model: "Hall",
        status: "pass",
        states: 3,
        holds: &["hallKnown"],
        broken: &[],
        findings: &[],
    },
    Expected {
        file: "tally.model.ts",
        model: "Tally",
        status: "fail",
        states: 5,
        holds: &["bounded"],
        broken: &[],
        findings: &[(
            "out-of-range",
            Shortest::Steps(&["add", "add", "add", "add"]),
        )],
    },
    Expected {
        file: "ticket.model.ts",
        model: "Ticket",
        status: "pass",
        states: 12,
        holds: &["seated"],
        broken: &[],
        findings: &[],
    },
    Expected {
        file: "trail.model.ts",
        model: "Trail",
        status: "fail",
        states: 7,
        holds: &["ends", "kept"],
        broken: &[("ordered", Shortest::Steps(&["visit", "visit"]))],
        findings: &[],
    },
    Expected {
        file: "turnstile.model.ts",
        model: "Turnstile",
        status: "fail",
        states: 4,
        holds: &[],
        broken: &[("locked", Shortest::Steps(&["push"]))],
        findings: &[],
    },
    Expected {
        file: "upload.model.ts",
        model: "Upload",
        status: "pass",
        states: 16,
        holds: &["quiet", "waiting", "counted"],
        broken: &[],
        findings: &[],
    },
];

pub(crate) const TWINS: &[(&str, &str, &str)] = &[
    ("Counter", "counter.model.ts", "counter.rs"),
    ("Download", "download.model.ts", "download.rs"),
    ("Mailbox", "mailbox.model.ts", "mailbox.rs"),
    ("Oven", "oven.model.ts", "oven.rs"),
    ("Turnstile", "turnstile.model.ts", "turnstile.rs"),
];

pub(crate) const OTHER_TYPESCRIPT: &[&str] = &[
    "limits/slow.model.ts",
    "changes/before/tank.model.ts",
    "changes/after/tank.model.ts",
];

pub(crate) fn files() -> Vec<&'static str> {
    let mut files: Vec<&str> = MODELS.iter().map(|expected| expected.file).collect();
    files.dedup();
    files
}

pub(crate) fn count(status: &str) -> u64 {
    let matching = MODELS
        .iter()
        .filter(|expected| expected.status == status)
        .count();
    u64::try_from(matching).expect("the table is small")
}

pub(crate) fn expected(model: &str) -> &'static Expected {
    MODELS
        .iter()
        .find(|expected| expected.model == model)
        .unwrap_or_else(|| panic!("no expected values for model {model}"))
}

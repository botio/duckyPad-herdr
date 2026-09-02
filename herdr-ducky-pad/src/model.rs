//! Pure logic for the duckyPad bridge: agent-state -> color and agent -> slot
//! assignment. No I/O here, so it is fully unit-testable.

/// Semantic herdr agent state (mirrors herdr's `AgentStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Working,
    Blocked,
    Done,
    Idle,
    Unknown,
}

impl AgentState {
    /// Parse herdr's `agent_status` string (`idle|working|blocked|done|unknown`).
    pub fn parse(s: &str) -> Self {
        match s {
            "working" => AgentState::Working,
            "blocked" => AgentState::Blocked,
            "done" => AgentState::Done,
            "idle" => AgentState::Idle,
            _ => AgentState::Unknown,
        }
    }

    /// Built-in RGB for this state (the locked default palette).
    pub fn color(self) -> [u8; 3] {
        match self {
            AgentState::Blocked => [255, 0, 0],
            AgentState::Working => [0, 255, 0],
            AgentState::Done => [0, 0, 255],
            AgentState::Unknown => [255, 165, 0],
            AgentState::Idle => [48, 48, 48],
        }
    }

    /// RGB for this state after applying a user palette override
    /// (`state name -> [r, g, b]`). Missing keys keep the built-in color.
    pub fn color_with(
        self,
        overrides: &std::collections::HashMap<String, [u8; 3]>,
    ) -> [u8; 3] {
        let name = match self {
            AgentState::Blocked => "blocked",
            AgentState::Working => "working",
            AgentState::Done => "done",
            AgentState::Unknown => "unknown",
            AgentState::Idle => "idle",
        };
        overrides
            .get(name)
            .copied()
            .unwrap_or_else(|| self.color())
    }
}

/// Number of lit keys (NeoPixels) the duckyPad exposes.
pub const SLOTS: usize = 15;

/// One herdr agent as the daemon tracks it.
#[derive(Debug, Clone)]
pub struct Agent {
    pub pane_id: String,
    pub name: String,
    pub state: AgentState,
}

impl Agent {
    /// Build from an `AgentInfo` JSON object (the elements of `agent.list`'s
    /// `agents` array). Returns `None` if a required field is missing.
    pub fn from_value(v: &serde_json::Value) -> Option<Agent> {
        let pane_id = v.get("pane_id")?.as_str()?.to_string();
        let state = v
            .get("agent_status")
            .and_then(|s| s.as_str())
            .map(AgentState::parse)
            .unwrap_or(AgentState::Unknown);
        let strf = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        Some(Agent {
            pane_id,
            name: pick_name(
                &strf("display_agent"),
                &strf("name"),
                &strf("agent"),
                &strf("label"),
            ),
            state,
        })
    }
}

fn pick_name(a: &str, b: &str, c: &str, d: &str) -> String {
    for s in [a, b, c, d] {
        let t = s.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    "agent".to_string()
}

/// Sticky assignment of herdr agents to the 15 pad keys.
///
/// Identity is `pane_id`. By default a new agent takes the lowest free slot,
/// visited in `agent.list` order; an existing agent keeps its slot no matter
/// how the list reorders or how states change; a slot frees when its agent
/// disappears.
///
/// `update` accepts an optional `pinned` map (slot 1..15 -> pane_id). A pinned
/// agent is kept on its pinned slot, overriding the sticky rule. `update` is
/// idempotent for an unchanged list, so it can be called on every pad push and
/// on every key press without side effects.
#[derive(Debug, Clone, Default)]
pub struct SlotMap {
    assign: std::collections::HashMap<String, usize>,
}

impl SlotMap {
    /// Reconcile with a fresh `agent.list`. Returns the 15 slots as
    /// `Vec<Option<&Agent>>` (index = key number - 1); overflow agents
    /// (more than 15 in the list) stay unlit until a slot frees.
    ///
    /// `pinned` optionally maps a slot (1..15) to a pane_id; pinned agents
    /// keep that slot, overriding the sticky lowest-free-slot rule.
    pub fn update<'a>(
        &mut self,
        agents: &'a [Agent],
        pinned: &std::collections::HashMap<usize, String>,
    ) -> Vec<Option<&'a Agent>> {
        let seen: std::collections::HashSet<&str> =
            agents.iter().map(|a| a.pane_id.as_str()).collect();
        self.assign
            .retain(|pane_id, _| seen.contains(pane_id.as_str()));

        let mut used = [false; SLOTS];
        for &slot in self.assign.values() {
            used[slot] = true;
        }

        // Pass 1: honor pins. A pinned agent takes its pinned slot if it is
        // free (or already ours). An un-pinned agent takes the lowest free
        // slot. Existing agents keep their slot unless a pin moves them.
        for a in agents {
            let target = match self.assign.get(&a.pane_id) {
                Some(&slot) => match pin_slot(pinned, &a.pane_id) {
                    Some(p) => p, // pin overrides sticky
                    None => slot, // keep sticky
                },
                None => match pin_slot(pinned, &a.pane_id) {
                    Some(p) if !used[p] => p,
                    _ => used.iter().position(|u| !*u).unwrap_or(SLOTS),
                },
            };
            if target < SLOTS && !used[target] {
                // release old slot, take new
                if let Some(&old) = self.assign.get(&a.pane_id) {
                    used[old] = false;
                }
                used[target] = true;
                self.assign.insert(a.pane_id.clone(), target);
            }
        }

        // Pass 2: any agent whose slot was stolen by a pin (collision) takes
        // the lowest free slot.
        for a in agents {
            if let Some(&slot) = self.assign.get(&a.pane_id) {
                // If another pinned agent claims this slot, move us.
                let claimed_by_other = pin_slot(pinned, &a.pane_id) != Some(slot)
                    && pinned.values().any(|p| {
                        let other_slot = pin_slot(pinned, p);
                        other_slot == Some(slot) && p != &a.pane_id
                    });
                if claimed_by_other {
                    let free = used.iter().position(|u| !*u).unwrap_or(SLOTS);
                    if free < SLOTS {
                        used[slot] = false;
                        used[free] = true;
                        self.assign.insert(a.pane_id.clone(), free);
                    }
                }
            }
        }

        let mut slots: Vec<Option<&Agent>> = vec![None; SLOTS];
        for a in agents {
            if let Some(&slot) = self.assign.get(&a.pane_id) {
                slots[slot] = Some(a);
            }
        }
        slots
    }
}

/// The pinned slot (0-indexed) for a pane_id, if any.
fn pin_slot(
    pinned: &std::collections::HashMap<usize, String>,
    pane_id: &str,
) -> Option<usize> {
    for (slot, pane) in pinned {
        if *pane == pane_id && (1..=SLOTS).contains(slot) {
            return Some(slot - 1);
        }
    }
    None
}

/// Encode the 15-slot frame as 45 bytes: key `i` -> bytes `[i*3, i*3+1, i*3+2]`
/// as (R,G,B). Unlit slots are (0,0,0).
///
/// `palette` is an optional user color override; pass an empty map to use the
/// built-in palette.
pub fn rgb_frame(slots: &[Option<&Agent>], palette: &std::collections::HashMap<String, [u8; 3]>) -> [u8; 45] {
    let mut out = [0u8; 45];
    for (i, slot) in slots.iter().enumerate().take(SLOTS) {
        let [r, g, b] = match slot {
            Some(a) => a.state.color_with(palette),
            None => [0, 0, 0],
        };
        out[i * 3] = r;
        out[i * 3 + 1] = g;
        out[i * 3 + 2] = b;
    }
    out
}

/// Build the OLED text (<= 56 bytes, `\n`-separated) listing the mapped agents
/// as `N:name`. The 6x10 font fits 20 chars / 6 lines on the 128x64 panel; we
/// stay well under the byte budget.
pub fn oled_text(slots: &[Option<&Agent>]) -> String {
    const LINE_LEN: usize = 18;
    const MAX_LINES: usize = 4;
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for (i, slot) in slots.iter().enumerate().take(SLOTS) {
        let Some(a) = slot else { continue };
        let name: String = a.name.chars().take(9).collect();
        let item = format!("{}:{} ", i + 1, name);
        if cur.len() + item.len() > LINE_LEN && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            if lines.len() >= MAX_LINES {
                return truncate_bytes(&lines.join("\n"), 56);
            }
        }
        cur.push_str(&item);
    }
    if !cur.trim().is_empty() {
        lines.push(cur);
    }
    lines.truncate(MAX_LINES);
    truncate_bytes(&lines.join("\n"), 56)
}

fn truncate_bytes(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn agent(pane: &str, state: AgentState) -> Agent {
        Agent {
            pane_id: pane.to_string(),
            name: pane.to_string(),
            state,
        }
    }

    fn no_pins() -> HashMap<usize, String> {
        HashMap::new()
    }

    #[test]
    fn state_color_locked() {
        assert_eq!(AgentState::Blocked.color(), [255, 0, 0]);
        assert_eq!(AgentState::Working.color(), [0, 255, 0]);
        assert_eq!(AgentState::Done.color(), [0, 0, 255]);
        assert_eq!(AgentState::Unknown.color(), [255, 165, 0]);
        assert_eq!(AgentState::Idle.color(), [48, 48, 48]);
    }

    #[test]
    fn state_color_with_override() {
        let mut p = HashMap::new();
        p.insert("working".to_string(), [10, 20, 30]);
        assert_eq!(AgentState::Working.color_with(&p), [10, 20, 30]);
        // non-overridden state keeps built-in
        assert_eq!(AgentState::Blocked.color_with(&p), [255, 0, 0]);
    }

    #[test]
    fn new_agents_take_lowest_free_slots_in_list_order() {
        let mut map = SlotMap::default();
        let agents = vec![
            agent("p-idle", AgentState::Idle),
            agent("p-blocked", AgentState::Blocked),
            agent("p-working", AgentState::Working),
            agent("p-done", AgentState::Done),
        ];
        let slots = map.update(&agents, &no_pins());
        // List order, NOT state priority: the first agent in the list gets
        // slot 0 even though it is only idle.
        assert_eq!(slots[0].unwrap().pane_id, "p-idle");
        assert_eq!(slots[1].unwrap().pane_id, "p-blocked");
        assert_eq!(slots[2].unwrap().pane_id, "p-working");
        assert_eq!(slots[3].unwrap().pane_id, "p-done");
    }

    #[test]
    fn state_changes_do_not_move_slots() {
        let mut map = SlotMap::default();
        let a = vec![
            agent("p-idle", AgentState::Idle),
            agent("p-working", AgentState::Working),
        ];
        let _ = map.update(&a, &no_pins());
        // Same agents, states flipped, list order kept: nobody moves.
        let b = vec![
            agent("p-idle", AgentState::Working),
            agent("p-working", AgentState::Idle),
        ];
        let slots = map.update(&b, &no_pins());
        assert_eq!(slots[0].unwrap().pane_id, "p-idle");
        assert_eq!(slots[1].unwrap().pane_id, "p-working");
    }

    #[test]
    fn pinned_agent_keeps_its_slot() {
        let mut map = SlotMap::default();
        let mut pins = HashMap::new();
        pins.insert(3usize, "p-pinned".to_string());
        let agents = vec![
            agent("p-idle", AgentState::Idle),
            agent("p-pinned", AgentState::Working),
            agent("p-other", AgentState::Done),
        ];
        let slots = map.update(&agents, &pins);
        // p-idle takes slot 0 (lowest free), p-pinned pinned to slot 2,
        // p-other takes slot 1 (lowest remaining free).
        assert_eq!(slots[0].unwrap().pane_id, "p-idle");
        assert_eq!(slots[1].unwrap().pane_id, "p-other");
        assert_eq!(slots[2].unwrap().pane_id, "p-pinned");
    }

    #[test]
    fn pin_overrides_sticky_on_relist() {
        let mut map = SlotMap::default();
        // First: no pins, p-a takes slot 0.
        let a1 = vec![agent("p-a", AgentState::Idle)];
        let _ = map.update(&a1, &no_pins());
        assert_eq!(map.assign.get("p-a"), Some(&0));
        // Now pin p-a to slot 5.
        let mut pins = HashMap::new();
        pins.insert(5usize, "p-a".to_string());
        let slots = map.update(&a1, &pins);
        assert_eq!(slots[4].unwrap().pane_id, "p-a");
    }

    #[test]
    fn empty_palette_keeps_builtin() {
        let a = agent("x", AgentState::Working);
        let slots: Vec<Option<&Agent>> = vec![Some(&a)];
        let rgb = rgb_frame(&slots, &HashMap::new());
        // slot 0 -> bytes [0,1,2]
        assert_eq!([rgb[0], rgb[1], rgb[2]], [0, 255, 0]);
    }
    #[test]
    fn palette_override_changes_frame() {
        let mut p = HashMap::new();
        p.insert("working".to_string(), [1, 2, 3]);
        let a = agent("x", AgentState::Working);
        let slots: Vec<Option<&Agent>> = vec![Some(&a)];
        let rgb = rgb_frame(&slots, &p);
        assert_eq!([rgb[0], rgb[1], rgb[2]], [1, 2, 3]);
    }
}

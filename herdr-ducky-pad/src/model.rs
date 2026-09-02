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

    /// Slot-fill priority rank: lower fills slots first.
    /// `blocked > done > working > unknown > idle`.
    pub fn rank(self) -> u8 {
        match self {
            AgentState::Blocked => 0,
            AgentState::Done => 1,
            AgentState::Working => 2,
            AgentState::Unknown => 3,
            AgentState::Idle => 4,
        }
    }

    /// RGB for this state (locked palette).
    pub fn color(self) -> [u8; 3] {
        match self {
            AgentState::Blocked => [255, 0, 0],
            AgentState::Working => [0, 255, 0],
            AgentState::Done => [0, 0, 255],
            AgentState::Unknown => [255, 165, 0],
            AgentState::Idle => [48, 48, 48],
        }
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

/// Assign agents to the 15 slots by state priority. Overflow agents are dropped
/// (their keys stay unlit). Ties break by `pane_id` for determinism.
pub fn assign_slots<'a>(agents: &'a [Agent]) -> Vec<Option<&'a Agent>> {
    let mut order: Vec<&Agent> = agents.iter().collect();
    order.sort_by(|a, b| {
        a.state
            .rank()
            .cmp(&b.state.rank())
            .then_with(|| a.pane_id.cmp(&b.pane_id))
    });
    let mut slots: Vec<Option<&Agent>> = vec![None; SLOTS];
    for (i, a) in order.iter().enumerate() {
        if i < SLOTS {
            slots[i] = Some(*a);
        }
    }
    slots
}

/// Encode the 15-slot frame as 45 bytes: key `i` -> bytes `[3+i*3, 4+i*3, 5+i*3]`
/// as (R,G,B). Unlit slots are (0,0,0).
pub fn rgb_frame(slots: &[Option<&Agent>]) -> [u8; 45] {
    let mut out = [0u8; 45];
    for (i, slot) in slots.iter().enumerate().take(SLOTS) {
        let [r, g, b] = match slot {
            Some(a) => a.state.color(),
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

    fn agent(pane: &str, state: AgentState) -> Agent {
        Agent {
            pane_id: pane.to_string(),
            name: pane.to_string(),
            state,
        }
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
    fn state_priority_blocked_first() {
        let agents = vec![
            agent("p-idle", AgentState::Idle),
            agent("p-blocked", AgentState::Blocked),
            agent("p-working", AgentState::Working),
            agent("p-done", AgentState::Done),
        ];
        let slots = assign_slots(&agents);
        assert_eq!(slots[0].unwrap().pane_id, "p-blocked");
        assert_eq!(slots[1].unwrap().pane_id, "p-done");
        assert_eq!(slots[2].unwrap().pane_id, "p-working");
        assert_eq!(slots[3].unwrap().pane_id, "p-idle");
    }

    #[test]
    fn overflow_beyond_15_is_unlit() {
        let agents: Vec<Agent> = (0..20)
            .map(|i| agent(&format!("p{i}"), AgentState::Working))
            .collect();
        let slots = assign_slots(&agents);
        assert_eq!(slots.len(), 15);
        assert!(slots.iter().all(|s| s.is_some()));
        assert_eq!(rgb_frame(&slots).len(), 45);
    }

    #[test]
    fn rgb_frame_layout() {
        let agents = vec![
            agent("a", AgentState::Blocked),
            agent("b", AgentState::Working),
        ];
        let slots = assign_slots(&agents);
        let f = rgb_frame(&slots);
        assert_eq!([f[0], f[1], f[2]], [255, 0, 0]); // slot 0 blocked
        assert_eq!([f[3], f[4], f[5]], [0, 255, 0]); // slot 1 working
        assert_eq!([f[6], f[7], f[8]], [0, 0, 0]); // slot 2 unlit
    }

    #[test]
    fn oled_text_fits_budget() {
        let agents: Vec<Agent> = (0..10)
            .map(|i| agent(&format!("agent{i}"), AgentState::Working))
            .collect();
        let slots = assign_slots(&agents);
        let t = oled_text(&slots);
        assert!(t.len() <= 56, "oled text is {} bytes", t.len());
        assert!(!t.is_empty());
    }

    #[test]
    fn from_value_parses_agentinfo() {
        let v = serde_json::json!({
            "pane_id": "w1:p2",
            "workspace_id": "w1",
            "tab_id": "w1:t1",
            "agent_status": "blocked",
            "agent": "codex",
            "display_agent": "Codex: auth",
            "name": null,
            "focused": true,
            "revision": 7
        });
        let a = Agent::from_value(&v).unwrap();
        assert_eq!(a.pane_id, "w1:p2");
        assert_eq!(a.state, AgentState::Blocked);
        assert_eq!(a.name, "Codex: auth");
    }
}

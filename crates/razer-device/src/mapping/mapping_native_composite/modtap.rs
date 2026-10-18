use super::*;

struct Pending {
    deadline: Duration,
    make: Resolved<Output>,
    tap: Output,
    hold: Output,
}

#[derive(Default)]
pub(super) struct State {
    pending: VecDeque<Pending>,
}

impl State {
    pub(super) fn apply(
        &mut self,
        resolved: Resolved<Output>,
        now: Duration,
    ) -> Result<Vec<Dispatch>> {
        let ResolvedOutput::Mapped(Output::ModTap {
            milliseconds,
            tap,
            hold,
        }) = &resolved.output
        else {
            unreachable!("ModTap owner receives a typed ModTap output");
        };
        let id = resolved.input.stateless_id();
        let pending = self
            .pending
            .iter()
            .position(|row| row.make.input.stateless_id() == id);
        match resolved.input.phase() {
            Phase::Press => {
                if pending.is_some() {
                    bail!("ModTap press already has a pending timer");
                }
                self.pending.push_back(Pending {
                    deadline: now.saturating_add(Duration::from_millis((*milliseconds).into())),
                    tap: (**tap).clone(),
                    hold: (**hold).clone(),
                    make: resolved,
                });
                Ok(Vec::new())
            }
            Phase::Release => {
                if let Some(index) = pending {
                    let pending = self
                        .pending
                        .remove(index)
                        .expect("Located ModTap pending press");
                    // 8ba50 clones the stored make mapping, then the actual retained break mapping.
                    Ok(vec![
                        dispatch(&pending.make, pending.tap, None),
                        dispatch(&resolved, (**tap).clone(), None),
                    ])
                } else {
                    Ok(vec![dispatch(&resolved, (**hold).clone(), None)])
                }
            }
            Phase::Instant => Ok(Vec::new()),
        }
    }

    pub(super) fn poll(&mut self, now: Duration) -> Vec<Dispatch> {
        let mut output = Vec::new();
        while let Some((index, deadline)) = self
            .pending
            .iter()
            .enumerate()
            .min_by_key(|(_, row)| row.deadline)
            .map(|(index, row)| (index, row.deadline))
        {
            if deadline > now {
                break;
            }
            let row = self.pending.remove(index).expect("Located ModTap deadline");
            output.push(dispatch(&row.make, row.hold, None));
        }
        output
    }

    pub(super) fn clear(&mut self) {
        self.pending.clear();
    }
}

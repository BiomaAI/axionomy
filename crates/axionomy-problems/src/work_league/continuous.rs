//! Shared-time work economy. Events and worker decisions are proposed as
//! exchanges; time, deadlines, leases, activity duration and facility
//! ownership are laws. Each law is a rate with variables, serving every
//! worker, `{agent}`, and every contract, `{job}`, each role fixed to the
//! account its identifier names.

use super::*;

/// The season's clock: the ticks that have passed.
pub fn clock(world: &World) -> u64 {
    world.balance(&AccountId::World, &Asset::Tick).to_u64()
}
#[cfg(test)]
mod tests {
    use super::*;

    fn balance(world: &World, agent: AgentId, asset: Asset) -> u64 {
        world.balance(&AccountId::Agent(agent), &asset).to_u64()
    }
    fn job_balance(world: &World, id: JobId, asset: Asset) -> u64 {
        world.balance(&AccountId::Job(id), &asset).to_u64()
    }
    fn world() -> World {
        league(Profile::Showcase, mixed_lineup()).initial
    }
    fn act(world: &mut World, rate: RateId) {
        world.apply(action(world, rate)).unwrap();
    }
    fn tick(world: &mut World) {
        for agent in AGENTS {
            let wait = action(world, RateId::Wait { agent });
            if world.is_applicable(&wait) {
                world.apply(wait).unwrap();
            }
        }
        act(world, RateId::Tick);
    }
    fn until(world: &mut World, at: u64) {
        while clock(world) < at {
            tick(world);
        }
    }
    fn rejected_unchanged(world: &mut World, exchange: Action) {
        let before = world.state_key();
        assert!(world.apply(exchange).is_err());
        assert_eq!(world.state_key(), before);
    }
    fn refused(world: &mut World, rate: RateId) {
        let exchange = action(world, rate);
        rejected_unchanged(world, exchange);
    }

    #[test]
    fn workers_overlap_but_cannot_act_or_settle_early() {
        let mut world = world();
        for (agent, id) in [(AgentId::Atlas, JobId(1)), (AgentId::Bolt, JobId(2))] {
            act(&mut world, RateId::Claim { agent, job: id });
        }
        refused(
            &mut world,
            RateId::Claim {
                agent: AgentId::Coda,
                job: JobId(1),
            },
        );
        refused(&mut world, RateId::Tick);
        tick(&mut world);
        for (agent, to) in [
            (AgentId::Atlas, Location::North),
            (AgentId::Bolt, Location::East),
        ] {
            act(
                &mut world,
                RateId::Move {
                    agent,
                    from: Location::Depot,
                    to,
                },
            );
        }
        tick(&mut world);
        for (agent, id) in [(AgentId::Atlas, JobId(1)), (AgentId::Bolt, JobId(2))] {
            act(
                &mut world,
                RateId::Begin {
                    agent,
                    job: id,
                    mode: WorkMode::Rush,
                },
            );
        }
        assert_eq!(
            world
                .balance(&AccountId::World, &Asset::Activities)
                .to_u64(),
            2
        );
        let finish = RateId::Finish {
            agent: AgentId::Atlas,
            job: JobId(1),
            mode: WorkMode::Rush,
            outcome: Outcome::Success,
        };
        refused(&mut world, finish);
        tick(&mut world);
        refused(
            &mut world,
            RateId::Move {
                agent: AgentId::Atlas,
                from: Location::North,
                to: Location::Depot,
            },
        );
        until(&mut world, 4);
        act(&mut world, finish);
        assert_eq!(balance(&world, AgentId::Atlas, Asset::Completed), 1);
        assert_eq!(balance(&world, AgentId::Bolt, Asset::Busy), 1);
        assert_eq!(balance(&world, AgentId::Atlas, Asset::ElapsedTime), 4);
        assert_eq!(balance(&world, AgentId::Bolt, Asset::ElapsedTime), 4);
    }

    #[test]
    fn shared_service_slot_stays_occupied_for_its_duration() {
        let mut world = world();
        for agent in [AgentId::Atlas, AgentId::Bolt] {
            act(
                &mut world,
                RateId::Move {
                    agent,
                    from: Location::Depot,
                    to: Location::West,
                },
            );
        }
        tick(&mut world);
        for agent in [AgentId::Atlas, AgentId::Bolt] {
            act(
                &mut world,
                RateId::Move {
                    agent,
                    from: Location::West,
                    to: Location::Workshop,
                },
            );
        }
        tick(&mut world);
        let start = |agent| RateId::BeginService {
            agent,
            facility: Facility::Workshop,
            maintenance: true,
        };
        act(&mut world, start(AgentId::Atlas));
        refused(&mut world, start(AgentId::Bolt));
        let end = RateId::EndService {
            agent: AgentId::Atlas,
            facility: Facility::Workshop,
        };
        refused(&mut world, end);
        until(&mut world, 6);
        act(&mut world, end);
        act(&mut world, start(AgentId::Bolt));
        assert_eq!(
            world
                .balance(
                    &AccountId::Facility(Facility::Workshop),
                    &Asset::Occupied(Facility::Workshop, AgentId::Bolt)
                )
                .to_u64(),
            1
        );
        let elsewhere = action(&world, start(AgentId::Atlas))
            .bind(Role::Facility, AccountId::Facility(Facility::Charger));
        rejected_unchanged(&mut world, elsewhere);
    }

    #[test]
    fn travel_uses_the_declared_road_network_one_hop_a_turn() {
        let mut world = world();
        let agent = AgentId::Atlas;
        assert!(
            world
                .instance(&RateId::Move {
                    agent,
                    from: Location::Depot,
                    to: Location::Workshop
                })
                .is_err()
        );
        assert_eq!(
            world
                .rate_ids()
                .flat_map(|key| world.instances_of(key))
                .filter(|id| matches!(
                    id,
                    RateId::Move {
                        agent: AgentId::Atlas,
                        ..
                    }
                ))
                .count(),
            18
        );
        // The workshop is two roads from the depot, one hop a turn.
        act(
            &mut world,
            RateId::Move {
                agent,
                from: Location::Depot,
                to: Location::West,
            },
        );
        refused(
            &mut world,
            RateId::Move {
                agent,
                from: Location::West,
                to: Location::Workshop,
            },
        );
        tick(&mut world);
        act(
            &mut world,
            RateId::Move {
                agent,
                from: Location::West,
                to: Location::Workshop,
            },
        );
        assert_eq!(balance(&world, agent, Asset::At(Location::Workshop)), 1);
    }

    #[test]
    fn future_jobs_dependencies_leases_and_deadlines_are_core_enforced() {
        let mut world = world();
        refused(
            &mut world,
            RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(5),
            },
        );
        refused(
            &mut world,
            RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(4),
            },
        );
        act(
            &mut world,
            RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(1),
            },
        );
        let release = RateId::ReleaseClaim {
            agent: AgentId::Atlas,
            job: JobId(1),
            expired: true,
        };
        refused(&mut world, release);
        until(&mut world, 7);
        act(&mut world, release);
        assert_eq!(balance(&world, AgentId::Atlas, Asset::FreeContract), 1);
        act(
            &mut world,
            RateId::Claim {
                agent: AgentId::Bolt,
                job: JobId(1),
            },
        );
        let expiry = RateId::Expire {
            job: JobId(1),
            agent: Some(AgentId::Bolt),
            mode: None,
        };
        refused(&mut world, expiry);
        until(&mut world, 24);
        act(&mut world, expiry);
        assert_eq!(job_balance(&world, JobId(1), Asset::Expired), 1);
        assert_eq!(balance(&world, AgentId::Bolt, Asset::FreeContract), 1);
        refused(&mut world, expiry);
        refused(
            &mut world,
            RateId::Claim {
                agent: AgentId::Coda,
                job: JobId(1),
            },
        );
        refused(&mut world, RateId::EndSeason);
    }

    #[test]
    fn joint_handoff_moves_the_claim_and_payment_once() {
        let mut world = world();
        act(
            &mut world,
            RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(1),
            },
        );
        tick(&mut world);
        let handoff = RateId::Handoff {
            agent: AgentId::Atlas,
            recipient: AgentId::Bolt,
            job: JobId(1),
        };
        let wrong = action(&world, handoff).bind(Role::Partner, AccountId::Agent(AgentId::Coda));
        rejected_unchanged(&mut world, wrong);
        act(&mut world, handoff);
        assert_eq!(balance(&world, AgentId::Atlas, Asset::Credit), 7);
        assert_eq!(balance(&world, AgentId::Bolt, Asset::Credit), 3);
        assert_eq!(balance(&world, AgentId::Atlas, Asset::FreeContract), 1);
        assert_eq!(balance(&world, AgentId::Bolt, Asset::Claimed(JobId(1))), 1);
        assert_eq!(
            job_balance(&world, JobId(1), Asset::Assigned(AgentId::Bolt)),
            1
        );
        // The handoff spent Bolt's turn too.
        refused(
            &mut world,
            RateId::Wait {
                agent: AgentId::Bolt,
            },
        );
        tick(&mut world);
        refused(
            &mut world,
            RateId::Handoff {
                agent: AgentId::Bolt,
                recipient: AgentId::Atlas,
                job: JobId(1),
            },
        );
    }

    #[test]
    fn recycling_recovers_material_with_a_conserved_loss() {
        let mut world = world();
        let agent = AgentId::Atlas;
        act(
            &mut world,
            RateId::Claim {
                agent,
                job: JobId(1),
            },
        );
        tick(&mut world);
        act(
            &mut world,
            RateId::Move {
                agent,
                from: Location::Depot,
                to: Location::North,
            },
        );
        tick(&mut world);
        act(
            &mut world,
            RateId::Begin {
                agent,
                job: JobId(1),
                mode: WorkMode::Rush,
            },
        );
        until(&mut world, 4);
        act(
            &mut world,
            RateId::Finish {
                agent,
                job: JobId(1),
                mode: WorkMode::Rush,
                outcome: Outcome::Success,
            },
        );
        act(
            &mut world,
            RateId::Move {
                agent,
                from: Location::North,
                to: Location::Recycler,
            },
        );
        tick(&mut world);
        let material = balance(&world, agent, Asset::Material);
        act(
            &mut world,
            RateId::BeginService {
                agent,
                facility: Facility::Recycler,
                maintenance: false,
            },
        );
        assert_eq!(balance(&world, agent, Asset::Material), material);
        until(&mut world, 7);
        act(
            &mut world,
            RateId::EndService {
                agent,
                facility: Facility::Recycler,
            },
        );
        assert_eq!(balance(&world, agent, Asset::Material), material + 1);
        assert_eq!(balance(&world, agent, Asset::Waste), 0);
        assert_eq!(balance(&world, agent, Asset::RecyclingLoss), 1);
        assert_eq!(balance(&world, agent, Asset::RecycledWaste), 2);
    }

    #[test]
    fn a_late_work_completion_cannot_collect_payment() {
        let mut world = world();
        let agent = AgentId::Atlas;
        act(
            &mut world,
            RateId::Claim {
                agent,
                job: JobId(1),
            },
        );
        tick(&mut world);
        act(
            &mut world,
            RateId::Move {
                agent,
                from: Location::Depot,
                to: Location::North,
            },
        );
        tick(&mut world);
        act(
            &mut world,
            RateId::Begin {
                agent,
                job: JobId(1),
                mode: WorkMode::Rush,
            },
        );
        until(&mut world, 24);
        refused(
            &mut world,
            RateId::Finish {
                agent,
                job: JobId(1),
                mode: WorkMode::Rush,
                outcome: Outcome::Success,
            },
        );
        act(
            &mut world,
            RateId::Expire {
                job: JobId(1),
                agent: Some(agent),
                mode: Some(WorkMode::Rush),
            },
        );
        assert_eq!(balance(&world, agent, Asset::Credit), 5);
        assert_eq!(balance(&world, agent, Asset::Busy), 0);
        assert_eq!(
            world
                .balance(&AccountId::World, &Asset::Activities)
                .to_u64(),
            0
        );
    }
}

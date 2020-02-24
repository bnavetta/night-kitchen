# Night Kitchen

Night Kitchen is an (ab)use of systemd for running periodic tasks like backups or system cleanup.

## Features

* Responds to power events - Night Kitchen detects when the system is about to sleep or shut down and ensures that it starts up again before the next
  round of tasks. Then, when those tasks complete, it suspends or powers off the system again as needed.

* Extensible - Night Kitchen uses systemd [targets](https://www.freedesktop.org/software/systemd/man/systemd.target.html), so it's easy to add new tasks

## Components

### `night-kitchen-scheduler`

The scheduler daemon is mostly responsible for making sure the system is up when tasks are supposed to run. It uses
[inhibitor locks](https://www.freedesktop.org/wiki/Software/systemd/inhibit/) to schedule an [RTC alarm](https://en.wikipedia.org/wiki/Real-time_clock_alarm)
for the next timer activation whenever the system is about to shut down. Waking from suspend is handled by systemd through the `WakeSystem` timer setting.

It also records whenever the system wakes from suspend, so that the runner can decide if it needs to suspend again.

### `night-kitchen-runner`

The runner starts whatever task target corresponds to the timer that triggered it and then returns the system to the state it was originally in. For example, if
systemd woke from sleep to activate `night-kitchen-daily.timer`, `night-kitchen-runner` would start `night-kitchen-daily.target` and then put the system back to 
sleep.

To check if the system should be shut down, `night-kitchen-runner` compares the uptime to the time it started at. Similarly, it uses the resume timestamp from
`night-kitchen-scheduler` to decide if it should suspend.

### `night-kitchen-{daily,weekly}.timer`

These timers run once a day and once a week, respectively, and trigger oneshot services that start `night-kitchen-runner`. In addition, `night-kitchen-scheduler` 
uses them to set the RTC alarm.

### `night-kitchen-{daily,weekly}.target`

These targets group together tasks for Night Kitchen to run.
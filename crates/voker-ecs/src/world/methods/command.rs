use crate::command::Commands;
use crate::error::ErrorContext;
use crate::world::World;

impl World {
    /// Create a commands from world.
    pub fn commands(&self) -> Commands<'_> {
        Commands::new(self)
    }

    /// Drains and executes queued deferred commands.
    ///
    /// Each command failure is forwarded to the active default error handler,
    /// and command application continues with remaining commands.
    pub fn apply_commands(&mut self) {
        let handler = self.default_error_handler();

        while let Some(cmd) = self.command_queue.pop() {
            let location = cmd.location();
            if let Err(err) = cmd.run(self) {
                voker_utils::cold_path();
                let this_run = self.this_run();
                let ctx = ErrorContext::Command { location, this_run };
                (handler)(err, ctx);
            }
        }
    }
}

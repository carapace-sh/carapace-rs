//! Gcloud bridge: the argcomplete v1 (fd) protocol with update checks
//! disabled (port of Go `bridge.ActionGcloud`).

use carapace_rs::Action;

/// Bridge completions registered in gcloud (Google Cloud SDK).
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |_command| {
        Action::callback(move |c| {
            let mut context = c.clone();
            if context.value == "-" {
                // shorthand flags are not completed: expand to longhand
                context.value = "--".to_owned();
            }
            context.env.insert(
                "CLOUDSDK_COMPONENT_MANAGER_DISABLE_UPDATE_CHECK".to_owned(),
                "1".to_owned(),
            );
            super::argcomplete::v1(["gcloud"])
                .invoke(&context)
                .to_action()
        })
    })
}

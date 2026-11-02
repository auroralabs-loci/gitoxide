use bstr::{BString, ByteSlice};
use gix_credentials::{
    helper,
    protocol::{Context, ContextOptions},
};

use crate::helper::script_helper;

#[test]
fn follow_up_actions_discard_token_fields_rejected_in_wire_order() -> gix_testtools::TestResult {
    for caller in [false, true] {
        for accepted in [false, true] {
            for line_ending in ["\n", "\r\n"] {
                let response = format!(
                    "authtype=Rejected
credential=rejected-before-capability
ephemeral=1
capability[]=authtype
{}username=user
password=pass
quit=0
unknown=preserved

capability[]=authtype
authtype=Rejected
credential=rejected-after-record
ephemeral=1
",
                    if accepted {
                        "authtype=Bearer
credential=accepted-token
ephemeral=0
"
                    } else {
                        ""
                    }
                )
                .replace('\n', line_ending);
                let mut outcome = helper::invoke(
                    &mut gix_credentials::Program::from_custom_definition(format!(
                        "!f() {{ cat >/dev/null; printf '%s' '{response}'; }}; f"
                    ))?,
                    &helper::Action::Get(Context {
                        capability: if caller { vec!["authtype".into()] } else { Vec::new() },
                        ..Default::default()
                    }),
                )?
                .expect("the helper supplies at least username and password");
                let identity = outcome.consume_identity().expect("complete identity");
                assert_eq!(
                    identity.password,
                    if caller && accepted { "accepted-token" } else { "pass" },
                    "only fields accepted before the record terminator determine the identity"
                );
                for action in [outcome.next.clone().store(), outcome.next.erase()] {
                    let payload = action.payload().expect("follow-up actions have a payload");
                    assert!(
                        !payload.contains_str("rejected-"),
                        "discarded secrets must not be forwarded, even if a later token was accepted"
                    );
                    assert!(
                        payload.contains_str("accepted-token") == (caller && accepted),
                        "only negotiated token fields survive"
                    );
                    assert!(
                        payload.contains_str("unknown=preserved") && payload.contains_str("quit=0"),
                        "unrelated legacy fields remain verbatim"
                    );
                    let context = Context::from_bytes(payload, Default::default())?;
                    assert_eq!(
                        context.credential.as_deref(),
                        (caller && accepted).then_some("accepted-token"),
                        "stored credentials match the accepted response"
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn authtype_requires_caller_and_helper_negotiation_and_overrides_passwords() -> gix_testtools::TestResult {
    for caller in [false, true] {
        for responder in [false, true] {
            let response = format!(
                "{}authtype=Bearer
credential=access-token
ephemeral=1
username=user
password=pass
",
                if responder { "capability[]=authtype\n" } else { "" }
            );
            let mut outcome = helper::invoke(
                &mut gix_credentials::Program::from_custom_definition(format!(
                    "!f() {{ cat >/dev/null; printf '%s' '{response}'; }}; f"
                ))?,
                &helper::Action::Get(Context {
                    capability: if caller { vec!["authtype".into()] } else { Vec::new() },
                    url: Some("https://example.com".into()),
                    ..Default::default()
                }),
            )?
            .expect("the helper always supplies a password");
            let identity = outcome.consume_identity().expect("a complete identity is available");
            let negotiated = caller && responder;
            assert_eq!(
                identity.authtype.as_deref(),
                negotiated.then_some("Bearer"),
                "both parties must announce support"
            );
            assert_eq!(
                identity.password,
                if negotiated { "access-token" } else { "pass" },
                "a complete token takes precedence over the password"
            );
            for action in [outcome.next.clone().store(), outcome.next.erase()] {
                let payload = action.payload().expect("follow-up payload");
                if !negotiated {
                    assert!(
                        !payload
                            .windows(b"credential=".len())
                            .any(|bytes| bytes == b"credential="),
                        "unnegotiated token fields must not be forwarded to helpers"
                    );
                }
                let context = Context::from_bytes(payload, Default::default())?;
                assert_eq!(
                    context.credential.as_deref(),
                    negotiated.then_some("access-token"),
                    "follow-up actions use only negotiated credentials"
                );
                assert_eq!(
                    context.ephemeral,
                    negotiated.then_some(true),
                    "the ephemeral hint accompanies the credential"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn get() -> gix_testtools::TestResult {
    let mut outcome = gix_credentials::helper::invoke(
        &mut script_helper("last-pass"),
        &helper::Action::get_for_url("https://github.com/byron/gitoxide"),
    )?
    .expect("mock provides credentials");
    assert_eq!(
        outcome.consume_identity().expect("complete"),
        gix_sec::identity::Account {
            username: "user".into(),
            password: "pass".into(),
            authtype: None,
            oauth_refresh_token: None
        }
    );
    assert_eq!(
        outcome.next.store().payload().expect("store action has a payload"),
        "username=user\npassword=pass\nquit=1\n"
    );
    Ok(())
}

#[test]
fn get_uses_context_options_for_the_entire_exchange() -> gix_testtools::TestResult {
    let action = helper::Action::Get(Context::from_url(
        "https://github.com/byron/gitoxide",
        ContextOptions {
            protect_protocol: false,
        },
    ));
    let outcome = gix_credentials::helper::invoke(&mut script_helper("carriage-return"), &action)?
        .expect("mock provides credentials");

    assert_eq!(outcome.username.as_deref(), Some("user\rname"));
    let context: Context = (&outcome.next).try_into()?;
    assert_eq!(context.options, action.context().expect("get action").options);
    assert_eq!(context.username.as_deref(), Some("user\rname"));
    Ok(())
}

#[test]
fn store_and_reject() {
    let ctx = Context {
        url: Some("https://github.com/byron/gitoxide".into()),
        ..Default::default()
    };
    let ctxbuf = || -> BString {
        let mut buf = Vec::<u8>::new();
        ctx.write_to(&mut buf).expect("cannot fail");
        buf.into()
    };
    for action in [helper::Action::Store(ctxbuf()), helper::Action::Erase(ctxbuf())] {
        let outcome = gix_credentials::helper::invoke(&mut script_helper("last-pass"), &action).unwrap();
        assert!(
            outcome.is_none(),
            "store and erase have no outcome, they just shouldn't fail"
        );
    }
}

mod program {
    use crate::TestResult;
    use gix_credentials::{Program, helper, program::Kind};

    use crate::helper::script_helper;

    #[test]
    fn builtin() -> TestResult {
        // Other tests resolve fixture paths relative to the working directory, so change it only in a child.
        if gix_testtools::run_in_isolated_process()? {
            return Ok(());
        }
        let temp = gix_testtools::tempfile::tempdir()?;
        let _cwd = gix_testtools::set_current_dir(temp.path())?;
        let err = gix_credentials::helper::invoke(
            &mut Program::from_kind(Kind::Builtin).suppress_stderr(),
            &helper::Action::get_for_url("/path/without/scheme/fails/with/error"),
        )
        .expect_err("the builtin helper rejects a URL without a scheme");
        insta::assert_debug_snapshot!(gix_testtools::redact_debug_snapshot(&err, &[("exit code:", "exit status:")]), "this failure indicates we could launch the helper, even though it wasn't happy which is fine. It doesn't like the URL", @r#"
        Credentials helper failed

        Caused by:
            0: I/O error (Other)
            1: Credentials helper program failed, exit_code=128, exit_status="exit status: 128"
        "#);
        assert!(
            err.classify().next().is_none(),
            "an unknown helper exit status must not promise that retrying will help"
        );
        Ok(())
    }

    #[test]
    fn script() -> gix_testtools::TestResult {
        assert_eq!(
            gix_credentials::helper::invoke(
                &mut Program::from_custom_definition(
                    "!f() { test \"$1\" = get && echo \"password=pass\" && echo \"username=user\"; }; f"
                )?,
                &helper::Action::get_for_url("/does/not/matter"),
            )?
            .expect("present")
            .consume_identity()
            .expect("complete"),
            gix_sec::identity::Account {
                username: "user".into(),
                password: "pass".into(),
                authtype: None,
                oauth_refresh_token: None
            }
        );
        Ok(())
    }

    #[cfg(unix)] // needs executable bits to work
    #[test]
    fn path_to_helper_script() -> gix_testtools::TestResult {
        assert_eq!(
            gix_credentials::helper::invoke(
                &mut Program::from_custom_definition(
                    gix_path::into_bstr(gix_path::realpath(gix_testtools::fixture_path("custom-helper.sh"))?)?
                        .into_owned(),
                )?,
                &helper::Action::get_for_url("/does/not/matter"),
            )?
            .expect("present")
            .consume_identity()
            .expect("complete"),
            gix_sec::identity::Account {
                username: "user-script".into(),
                password: "pass-script".into(),
                authtype: None,
                oauth_refresh_token: None
            }
        );
        Ok(())
    }

    #[test]
    fn path_to_helper_as_script_to_workaround_executable_bits() -> TestResult {
        assert_eq!(
            gix_credentials::helper::invoke(
                &mut script_helper("custom-helper"),
                &helper::Action::get_for_url("/does/not/matter"),
            )?
            .expect("present")
            .consume_identity()
            .expect("complete"),
            gix_sec::identity::Account {
                username: "user-script".into(),
                password: "pass-script".into(),
                authtype: None,
                oauth_refresh_token: None
            }
        );
        Ok(())
    }
}

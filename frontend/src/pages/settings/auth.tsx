import { useState, useEffect } from "react";
import { useForm, FormProvider } from "react-hook-form";
import QRCode from "qrcode";

import { Button } from "@/components/ui/button";
import {
    FormControl,
    FormField,
    FormItem,
    FormLabel,
} from "@/components/ui/form";
import { Input, PasswordInput } from "@/components/ui/input";
import { CenterPage } from "@/components/ui/page";
import { Separator } from "@/components/ui/separator";
import { send_to_clipboard } from "@/utils";
import { useTimer } from "@/components/hooks/timers";

export function Auth() {
    return <CenterPage className="pt-4 max-w-xl">
        <PasswordUpdate />
        <Separator />
        <MFAUpdate />
        <Separator />
    </CenterPage>;
}

interface PasswordForm {
    current: string,
    updated: string,
    confirm: string,
}

function PasswordUpdate() {
    const [display, set_display] = useState(false);
    const [update_result, set_update_result] = useState({
        failed: false,
        message: ""
    });
    const {set, clear} = useTimer(() => {
        set_update_result({
            failed: false,
            message: "",
        });
    })

    const form = useForm<PasswordForm>({
        defaultValues: {
            current: "",
            updated: "",
            confirm: "",
        }
    });

    async function on_submit(data: PasswordForm) {
        set_update_result({
            failed: false,
            message: "",
        });
        clear();

        try {
            let body = JSON.stringify({
                type: "UpdatePassword",
                ...data
            });

            let response = await fetch("/settings/auth", {
                method: "PATCH",
                headers: {
                    "content-type": "application/json; charset=utf-8",
                    "content-length": body.length.toString(10),
                },
                body
            });

            let json = await response.json();

            switch (response.status) {
                case 200:
                    if (json.type === "UpdatedPassword") {
                        form.reset();

                        set_update_result({
                            failed: false,
                            message: "Updated Password",
                        });
                        set(3000);
                    } else {
                        console.error("unknown response:", response.status, json);

                        set_update_result({
                            failed: true,
                            message: "unknown response",
                        });
                    }
                    break;
                case 400:
                    if (json.error === "InvalidConfirm") {
                        console.log("confirm password does not match updated");

                        set_update_result({
                            failed: true,
                            message: "Invalid confirm provided. Make sure that \"updated\" and \"confirm\" are the same.",
                        });
                    } else {
                        console.error("unknown response:", response.status, json);

                        set_update_result({
                            failed: true,
                            message: "unknown response",
                        });
                    }
                    break;
                case 403:
                    if (json.error === "InvalidPassword") {
                        console.log("invalid password");

                        set_update_result({
                            failed: true,
                            message: "Invalid password provided",
                        });
                    } else {
                        console.error("unknown response:", response.status, json);

                        set_update_result({
                            failed: true,
                            message: "unknown response",
                        });
                    }
                    break;
                case 500:
                    console.log("server error", json);

                    set_update_result({
                        failed: true,
                        message: "server error",
                    });
                    break;
                default:
                    console.error("unknown response:", response.status, json);

                    set_update_result({
                        failed: true,
                        message: "unknown response",
                    });
                    break;
            }
        } catch (err) {
            console.error("error sending password update", err);

            set_update_result({
                failed: true,
                message: "client error",
            });
        }
    }

    return <div className="space-y-4">
        <div className="flex flex-row items-center">
            <h2 className="grow text-xl">Password</h2>
            <Button type="button" variant="secondary" onClick={() => {
                form.reset();

                set_display(v => !v);
            }}>
                {display ? "Hide" : "Change"}
            </Button>
        </div>
        {display ?
            <FormProvider<PasswordForm> {...form}>
                <form className="space-y-4" onSubmit={form.handleSubmit(on_submit)}>
                    <FormField control={form.control} name="current" render={({field}) => {
                        return <FormItem className="w-1/2">
                            <FormLabel>Current Password</FormLabel>
                            <FormControl>
                                <PasswordInput autoComplete="current-password" {...field}/>
                            </FormControl>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="updated" render={({field}) => {
                        return <FormItem className="w-1/2">
                            <FormLabel>New Password</FormLabel>
                            <FormControl>
                                <PasswordInput autoComplete="new-password" {...field}/>
                            </FormControl>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="confirm" render={({field}) => {
                        return <FormItem className="w-1/2">
                            <FormLabel>Confirm Password</FormLabel>
                            <FormControl>
                                <PasswordInput autoComplete="new-password" {...field}/>
                            </FormControl>
                        </FormItem>;
                    }}/>
                    <div className="flex flex-row gap-4 items-center">
                        <Button type="submit" disabled={!form.formState.isDirty || form.formState.isSubmitting}>Save</Button>
                        <span>{update_result.message}</span>
                    </div>
                </form>
            </FormProvider>
            :
            null
        }
    </div>;
}

function MFAUpdate() {
    return <div className="space-y-4">
        <h2 className="text-xl">Multi-Factor Authentication (MFA, 2FA)</h2>
        <div className="space-y-4">
            <TotpEdit />
        </div>
    </div>;
}

interface TotpEnabled {
    type: "enabled"
}

interface TotpDisabled {
    type: "disabled"
}

interface TotpVerify {
    type: "verify",
    algo: string,
    step: number,
    digits: number,
    secret: string,
    url: string,
    data_url: string,
}

type TotpState = TotpEnabled | TotpDisabled | TotpVerify;

function TotpEdit() {
    const [loading, set_loading] = useState(false);
    const [view_params, set_view_params] = useState(false);

    const [code, set_code] = useState("");

    const [state, set_state] = useState<TotpState>({type: "disabled"});

    async function fetch_totp() {
        set_loading(true);

        try {
            let res = await fetch("/settings/auth?kind=Totp")

            if (res.status === 200) {
                let json = await res.json();

                if (json.type === "Totp") {
                    set_state({type: json.enabled ? "enabled" : "disabled"});
                } else {
                    console.warn("unhandled response type", json.type);
                }
            } else {
                console.warn("unhandled response code");
            }
        } catch (err) {
            console.error("failed to retrieve totp settings", err);
        }

        set_loading(false);
    }

    async function enable_totp() {
        set_loading(true);

        try {
            let body = JSON.stringify({
                type: "EnableTotp",
            });
            let res = await fetch("/settings/auth", {
                method: "PATCH",
                headers: {
                    "content-type": "application/json",
                    "content-length": body.length.toString(10),
                },
                body
            });

            switch (res.status) {
                case 200:
                    let json = await res.json();

                    if (json.type !== "CreatedTotp") {
                        console.log("response type is not CreatedTotp");
                    } else {
                        let { algo, step, digits, secret } = json;
                        let url = `otpauth://totp/test?issuer=tj2&secret=${secret}&period=${step}&algorithm=${algo}`;

                        let data_url = await QRCode.toDataURL(url, {
                            type: "image/png",
                            //quality: 1,
                            margin: 1,
                            color: {
                                dark: "#010599FF",
                                light: "#FFBF60FF",
                            }
                        });

                        set_code("");
                        set_state({
                            type: "verify",
                            algo,
                            step,
                            digits,
                            secret,
                            url,
                            data_url,
                        });
                    }
                    break;
                default:
                    console.warn("unhandled status code");
                    break;
            }
        } catch (err) {
            console.error("failed to enable totp", err);
        }

        set_loading(false);
    }

    async function verify_totp() {
        set_loading(true);

        try {
            let body = JSON.stringify({
                type: "VerifyTotp",
                code: code
            });
            let res = await fetch("/settings/auth", {
                method: "PATCH",
                headers: {
                    "content-type": "application/json",
                    "content-length": body.length.toString(10),
                },
                body
            });

            switch (res.status) {
                case 200: {
                    let json = await res.json();

                    if (json.type === "VerifiedTotp") {
                        set_code("");
                        set_view_params(false);
                        set_state({type: "enabled"});
                    } else {
                        console.log("response type is not VerifiedTotp");
                    }

                    break;
                }
                case 400: {
                    let json = await res.json();

                    if (json.type === "InvalidTotpCode") {
                        console.log("Invalid totp code");
                    } else if (json.type === "TotpNotFound") {
                        console.log("totp was not found for user");
                    }

                    break;
                }
                default:
                    console.warn("unhandled status code");
                    break;
            }
        } catch (err) {
            console.error("error when verifying totp", err);
        }

        set_loading(false);
    }

    async function disable_totp() {
        set_loading(true);

        try {
            let body = JSON.stringify({
                type: "DisableTotp"
            });
            let res = await fetch("/settings/auth", {
                method: "PATCH",
                headers: {
                    "content-type": "application/json",
                    "content-length": body.length.toString(10),
                },
                body
            });

            switch (res.status) {
                case 200:
                    set_view_params(false);
                    set_state({type: "disabled"});
                    break;
                default:
                    console.warn("unhandled status code");
                    break;
            }
        } catch (err) {
            console.error("error when disabling totp");
        }

        set_loading(false);
    }

    useEffect(() => {
        fetch_totp();

        return () => {};
    }, []);

    let button;

    switch (state.type) {
        case "disabled":
            button = <Button
                type="button"
                variant="secondary"
                disabled={loading}
                onClick={() => enable_totp()}
            >
                Enable
            </Button>;
            break;
        case "enabled":
            button = <Button
                type="button"
                variant="destructive"
                disabled={loading}
                onClick={() => disable_totp()}
            >
                Disable
            </Button>;
            break;
        case "verify":
            button = <Button
                type="button"
                variant="secondary"
                disabled={loading}
                onClick={() => disable_totp()}
            >
                Cancel
            </Button>;
            break;
    }

    return <div className="rounded-lg border p-4 space-y-4">
        <div className="flex flex-row items-center justify-between">
            <div className="space-y-0.5">
                <span className="text-base">
                    Time One-Time-Password (TOTP)
                </span>
                <p className="text-xs">
                    Enable / Disable a Time based One-Time-Passwords when loging into the server
                </p>
            </div>
            {button}
        </div>
        {state.type === "verify" ?
            <>
                <div className="flex flex-row flex-nowrap gap-4">
                    <Button
                        type="button"
                        variant="ghost"
                        onClick={() => send_to_clipboard(state.url).then(() => {
                            console.log("wrote to clipboard");
                        }).catch(err => {
                            console.error("failed writing to clipboard", err);
                        })}
                    >
                        Copy URL
                    </Button>
                    <Button type="button" variant="ghost" onClick={() => set_view_params(v => !v)}>
                        {view_params ? "Hide Params" : "Show Params"}
                    </Button>
                </div>
                <div className="flex flex-row gap-2">
                    <img src={state.data_url}/>
                    {view_params ?
                        <div className="flex flex-col gap-2">
                            <span>algo: {state.algo}</span>
                            <span>period: {state.step}</span>
                            <span>digits: {state.digits}</span>
                            <span
                                className="hover:underline cursor-pointer"
                                onClick={() => send_to_clipboard(state.secret).then(() => {
                                    console.log("wrote to clipboard");
                                }).catch(err => {
                                    console.error("failed writing to clipboard", err);
                                })}
                            >
                                copy secret
                            </span>
                        </div>
                        :
                        null
                    }
                </div>
                <p className="text-xs">
                    You have 3 minutes to verify that your Authenticator works before the server discards the data.
                </p>
                <Input className="w-1/2" value={code} disabled={loading} onChange={e => {
                    set_code(e.target.value);
                }}/>
                <Button
                    type="button"
                    disabled={loading || code.length === 0}
                    onClick={() => verify_totp()}
                >
                    Verify
                </Button>
            </>
            :
            null
        }
    </div>;
}

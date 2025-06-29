import { format } from "date-fns";
import { useState, useEffect } from "react";
import { useForm, FormProvider } from "react-hook-form";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import QRCode from "qrcode";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
    FormControl,
    FormField,
    FormItem,
    FormLabel,
} from "@/components/ui/form";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
} from "@/components/ui/dialog";
import { Input, PasswordInput } from "@/components/ui/input";
import { CenterPage } from "@/components/ui/page";
import { Separator } from "@/components/ui/separator";
import { cn, send_to_clipboard } from "@/utils";
import { useTimer } from "@/components/hooks/timers";
import { ApiError, req_api_json } from "@/net";

export function Auth() {
    return <CenterPage className="pt-4 max-w-xl">
        <PasswordUpdate />
        <Separator />
        <MFAUpdate />
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
    const [set_timer, clear_timer] = useTimer(() => {
        set_update_result({
            failed: false,
            message: "",
        });
    });

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
        clear_timer();

        try {
            let json = await req_api_json("PATCH", "/settings/auth", {
                type: "UpdatePassword",
                ...data
            });

            if (json.type !== "UpdatedPassword") {
                throw new Error(`unknown json.type from server: ${json.type}`);
            }

            form.reset();

            set_update_result({
                failed: false,
                message: "Updated Password",
            });
            set_timer(3000);
        } catch (err) {
            if (err instanceof ApiError) {
                switch (err.kind) {
                    case "InvalidConfirm":
                        set_update_result({
                            failed: true,
                            message: "Invalid confirm provided. Make sure that \"updated\" and \"confirm\" are the same.",
                        });
                        break;
                    case "InvalidPassword":
                        set_update_result({
                            failed: true,
                            message: "Invalid password provided",
                        });
                        break;
                    default:
                        set_update_result({
                            failed: true,
                            message: "unknown response",
                        });
                }
            } else {
                console.error("error sending password update", err);

                set_update_result({
                    failed: true,
                    message: "client error",
                });
            }
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

interface MFAData {
    type: "MFA",
    totp: boolean,
    recovery: (string | null)[] | null
}

type AuthQuery = MFAData;

function MFAUpdate() {
    const client = useQueryClient();
    const {data, isError, isLoading} = useQuery({
        queryKey: ["mfa_query"],
        initialData: {
            type: "MFA",
            totp: false,
            recovery: null
        },
        queryFn: async () => {
            let json = await req_api_json<AuthQuery>("GET", "/settings/auth?kind=MFA");

            if (json.type !== "MFA") {
                throw new Error(`unknown json.type from server: ${json.type}`);
            }

            return json;
        }
    });

    let contents;

    if (isLoading) {
        contents = <div>Loading...</div>;
    } else if (isError) {
        contents = <div>Failed to load MFA data</div>;
    } else {
        contents = <>
            <TotpEdit enabled={data.totp} on_update={state => {
                client.setQueryData(
                    ["mfa_query"],
                    state ? 
                        {type: "MFA", totp: true, recovery: null} :
                        {type: "MFA", totp: false, recovery: null}
                );
            }}/>
            <RecoveryEdit allowed={data.totp} used_on={data.recovery} on_update={state => {
                client.setQueryData(
                    ["mfa_query"],
                    state ?
                        {type: "MFA", totp: true, recovery: [null, null, null, null, null]} :
                        {type: "MFA", totp: true, recovery: null}
                );
            }}/>
        </>
    }

    return <div className="space-y-4">
        <h2 className="text-xl">Multi-Factor Authentication (MFA, 2FA)</h2>
        <div className="space-y-4">
            {contents}
        </div>
    </div>;
}

interface TotpEditProps {
    enabled: boolean,
    on_update: (state: boolean) => void,
}

function TotpEdit({enabled, on_update}: TotpEditProps) {
    const [loading, set_loading] = useState(false);
    const [view_params, set_view_params] = useState(false);

    const [code, set_code] = useState("");
    const [verify_err, set_verify_err] = useState<string | null>(null);

    const [state, set_state] = useState<TotpState>(enabled ? {type: "enabled"} : {type: "disabled"});

    async function enable_totp() {
        set_loading(true);

        try {
            let json = await req_api_json("PATCH", "/settings/auth", {
                type: "EnableTotp"
            });

            if (json.type !== "EnabledTotp") {
                throw new Error(`unknown json type from server: ${json.type}`);
            }

            let { algo, step, digits, secret } = json;
            let url = `otpauth://totp/test?issuer=tj2&secret=${secret}&period=${step}&algorithm=${algo}`;

            let data_url = await QRCode.toDataURL(url, {
                type: "image/png",
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
        } catch (err) {
            console.error("failed to enable totp", err);

            toast("Failed to enable Totp.");
        }

        set_loading(false);
    }

    async function verify_totp() {
        set_loading(true);

        try {
            let json = await req_api_json("PATCH", "/settings/auth", {
                type: "VerifyTotp",
                code: code
            });

            if (json.type !== "VerifiedTotp") {
                throw new Error(`unknown json type from server: ${json.type}`);
            }

            on_update(true);
        } catch (err) {
            if (err instanceof ApiError) {
                if (err.kind === "InvalidTotpCode") {
                    set_verify_err("Invalid Totp code");
                } else if (err.kind === "TotpNotFound") {
                    set_verify_err("Totp is no longer available. Try disabling and re-enabling");
                } else {
                    console.error("error when verifying totp", err);

                    set_verify_err("error when sending verification code.");
                }
            } else {
                console.error("error when verifying totp", err);

                set_verify_err("error when sending verification code.");
            }
        }

        set_loading(false);
    }

    async function disable_totp() {
        set_loading(true);

        try {
            let json = await req_api_json("PATCH", "/settings/auth", {
                type: "DisableTotp"
            });

            if (json.type !== "DisabledTotp" && json.type !== "Noop") {
                throw new Error(`unknown json.type from server: ${json.type}`);
            }

            if (enabled) {
                on_update(false);
            } else {
                set_state({type: "disabled"});
                set_code("");
                set_verify_err(null);
                set_view_params(false);
            }
        } catch (err) {
            console.error("error when disabling totp", err);

            toast("Failed to disable Totp.");
        }

        set_loading(false);
    }

    useEffect(() => {
        set_state(enabled ? {type: "enabled"} : {type: "disabled"});
        set_code("");
        set_verify_err(null);
        set_view_params(false);
    }, [enabled]);

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

    return <div className="rounded-lg border space-y-4">
        <div className={cn("flex flex-row gap-2 items-start justify-between", {"p-4": state.type !== "verify", "pt-4 px-4": state.type === "verify"})}>
            <div className="space-y-0.5">
                <span className="text-base">
                    Time One-Time-Password (TOTP)
                </span>
                <p className="text-sm">
                    Enable / Disable a Time based One-Time-Passwords when loging into the server
                </p>
            </div>
            {button}
        </div>
        {state.type === "verify" ?
            <>
                <Separator/>
                <div className="pb-4 px-4 space-y-4">
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
                    <p className="text-sm">
                        You have 3 minutes to verify that your Authenticator works before the server discards the data.
                    </p>
                    <div className="space-y-0.5">
                        <Input className="w-1/2" value={code} disabled={loading} onChange={e => {
                            set_code(e.target.value);
                            set_verify_err(null);
                        }}/>
                        {verify_err != null ?
                            <p className="text-sm font-medium text-destructive">{verify_err}</p>
                            :
                            null
                        }
                    </div>
                    <Button
                        type="button"
                        disabled={loading || code.length === 0}
                        onClick={() => verify_totp()}
                    >
                        Verify
                    </Button>
                </div>
            </>
            :
            null
        }
    </div>;
}

interface RecoveryEditProps {
    allowed: boolean,
    used_on: (string | null)[] | null,
    on_update: (state: boolean) => void,
}

function RecoveryEdit({allowed, used_on, on_update}: RecoveryEditProps) {
    const [loading, set_loading] = useState(false);

    const [codes, set_codes] = useState<string[] | null>(null);

    async function enable_recovery() {
        set_loading(true);

        try {
            let json = await req_api_json("PATCH", "/settings/auth", {
                type: "EnableRecovery"
            });

            if (json.type !== "EnabledRecovery") {
                throw new Error(`unknown json.type from server: ${json.type}`);
            }

            set_codes(json.codes);

            on_update(true);
        } catch (err) {
            if (err instanceof ApiError) {
                toast("failed to enable recovery codes");
            } else {
                toast("failed to enable recovery codes");
            }
        }

        set_loading(false);
    }

    async function disable_recovery() {
        set_loading(true);

        try {
            let json = await req_api_json("PATCH", "/settings/auth", {
                type: "DisableRecovery",
            });

            if (json.type !== "DisabledRecovery") {
                throw new Error(`unknown json.type from server: ${json.type}`);
            }

            on_update(false);
        } catch (err) {
            if (err instanceof ApiError) {
                toast("failed to disable recovery codes.");
            } else {
                toast("Failed to disable recovery codes.");
            }
        }

        set_loading(false);
    }

    useEffect(() => {
        if (!allowed) {
            set_codes(null);
        }
    }, [allowed]);

    let button;

    if (!allowed) {
        button = <Button
            type="button"
            variant="secondary"
            disabled={true}
        >
            Enable TOTP
        </Button>;
    } else if (allowed) {
        if (used_on == null) {
            button = <Button
                type="button"
                variant="secondary"
                disabled={loading}
                onClick={() => enable_recovery()}
            >
                Enable
            </Button>;
        } else {
            button = <Button
                type="button"
                variant="destructive"
                disabled={loading}
                onClick={() => disable_recovery()}
            >
                Disable
            </Button>;
        }
    }

    let used_on_list = [];

    for (let date of used_on ?? []) {
        if (date == null) {
            continue;
        }

        let fmt = format(date, "yyyy/MM/dd HH:mm:ss");

        used_on_list.push(<li key={date} title={date}>{fmt}</li>);
    }

    return <div className="rounded-lg border space-y-4">
        <div className={cn("flex flex-row gap-2 items-start justify-between", {"p-4": used_on == null, "pt-4 px-4": used_on != null})}>
            <div className="space-y-0.5">
                <span className="text-base">
                    Recovery Codes
                </span>
                <p className="text-sm">
                    Allow for the use of recovery codes in the event that a MFA method is not available (ex. loosing an authenticator).
                </p>
            </div>
            {button}
        </div>
        {used_on != null ?
            <>
                <Separator/>
                <div className="pb-4 px-4 space-y-0.5">
                {used_on_list.length > 0 ?
                    <>
                        <p className="text-xs">
                            A list of dates indicating when a recovery code was used.
                        </p>
                        <ul className="pl-4">
                            {used_on_list}
                        </ul>
                    </>
                    :
                    <p className="text-sm">
                        No codes have been used.
                    </p>
                }
                </div>
            </>
            :
            null
        }
        {codes != null ?
            <Dialog defaultOpen={true} onOpenChange={v => {
                if (!v) {
                    set_codes(null);
                }
            }}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Recovery Codes</DialogTitle>
                        <DialogDescription>
                            This is the list of recovery codes to use. DO NOT SHARE THESE. Treat them as passwords and place them somewhere safe. THESE WILL NOT BE SHOWN AGAIN.
                        </DialogDescription>
                    </DialogHeader>
                    <Separator/>
                    <div className="flex flex-col items-center space-y-4">
                        <ul>
                            {codes.map(code => {
                                return <li key={code}><pre>{code}</pre></li>
                            })}
                        </ul>
                        <Button type="button" variant={"secondary"} onClick={() => {
                            send_to_clipboard(codes.join("\n")).then(() => {
                                toast("Copied to clipboard");
                            }).catch(err => {
                                console.error("failed to copy codes to clipboard", err);

                                toast("Failed to copy to clipboard");
                            });
                        }}>
                            Copy to Clipboard
                        </Button>
                    </div>
                </DialogContent>
            </Dialog>
            :
            null
        }
    </div>
}
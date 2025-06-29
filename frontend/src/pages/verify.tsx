import { useState } from "react";
import { useForm, SubmitHandler } from "react-hook-form";
import { useNavigate, useLocation } from "react-router-dom";

import { send_json } from "@/net";

import {
    InputOTP,
    InputOTPGroup,
    InputOTPSlot,
} from "@/components/ui/input-otp"
import { Button } from "@/components/ui/button";
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage } from "@/components/ui/form";
import { PasswordInput } from "@/components/ui/input";

interface VerifyForm {
    code: string
}

enum MFAType {
    Totp = 0,
    Recovery = 1,
}

function get_mfa_type(given: MFAType) {
    switch (given) {
        case MFAType.Totp:
            return "Totp";
        case MFAType.Recovery:
            return "Recovery";
    }
}

export function Verify() {
    const navigate = useNavigate();
    const location = useLocation();

    const [mfa_type, set_mfa_type] = useState<MFAType>(MFAType.Totp);

    const verify_form = useForm<VerifyForm>({
        defaultValues: {
            code: "",
        }
    });

    const on_submit: SubmitHandler<VerifyForm> = async (data, event) => {
        try {
            verify_form.clearErrors();

            let res = await send_json("POST", "/verify", {type: get_mfa_type(mfa_type), ...data});
            let prev = new URL(location.pathname + location.search, window.location.origin)
                .searchParams
                .get("prev");

            switch (res.status) {
                case 200:
                    navigate(prev ?? "/journals");

                    break;
                case 400: {
                    let json = await res.json();

                    if (json.error === "InvalidCode") {
                        verify_form.setError("code", {type: "custom", message: "Invalid TOTP code."});
                    } else if (json.error === "InvalidRecovery") {
                        verify_form.setError("code", {type: "custom", message: "Invalid recovery code."});
                    } else if (json.error === "InvalidSession") {
                        verify_form.setError("code", {type: "custom", message: "Invalid session. Return to login and try again."})
                    } else if (json.error === "AlreadyVerified") {
                        navigate(prev ?? "/journals");
                    } else {
                        console.warn("unknown json.error:", json.error);
                    }

                    break;
                }
                case 404: {
                    let json = await res.json();

                    if (json.error === "MFANotFound") {
                        verify_form.setError("code", {type: "custom", message: "MFA not enabled for this account."});
                    } else {
                        console.warn("uknown json.error:", json.error);
                    }

                    break;
                }
                default:
                    console.warn("unhandled status code:", res.status);

                    break;
            }
        } catch (err) {
            console.error("failed to send verification", err);

            verify_form.setError("code", {type: "custom", message: "Error when sending verification. Try again."});
        }
    };

    let input;
    let btn_text;

    switch (mfa_type) {
        case MFAType.Totp:
            input = <FormField control={verify_form.control} name="code" render={({field}) => {
                return <FormItem>
                    <FormLabel>Time One-Time Password</FormLabel>
                    <FormControl>
                        <InputOTP maxLength={6} {...field}>
                            <InputOTPGroup>
                                <InputOTPSlot index={0} />
                                <InputOTPSlot index={1} />
                                <InputOTPSlot index={2} />
                                <InputOTPSlot index={3} />
                                <InputOTPSlot index={4} />
                                <InputOTPSlot index={5} />
                            </InputOTPGroup>
                        </InputOTP>
                    </FormControl>
                    <FormMessage/>
                </FormItem>;
            }}/>;
            btn_text = "Use Recovery";
            break;
        case MFAType.Recovery:
            input = <FormField control={verify_form.control} name="code" render={({field}) => {
                return <FormItem>
                    <FormLabel>Recovery Code</FormLabel>
                    <FormControl>
                        <PasswordInput {...field}/>
                    </FormControl>
                    <FormMessage/>
                </FormItem>
            }}/>;
            btn_text = "Use TOTP";
            break;
    }

    return <div className="flex w-full h-full">
        <div className="mx-auto my-auto">
            <Form {...verify_form} children={
                <form className="space-y-4" onSubmit={verify_form.handleSubmit(on_submit)}>
                    {input}
                    <div className="flex flex-row gap-x-4 justify-center">
                        <Button
                            type="button"
                            disabled={verify_form.formState.isSubmitting}
                            onClick={() => set_mfa_type(v => {
                                verify_form.reset();

                                if (v === MFAType.Recovery) {
                                    return MFAType.Totp;
                                } else {
                                    return MFAType.Recovery;
                                }
                            })}
                        >
                            {btn_text}
                        </Button>
                        <Button
                            type="submit"
                            disabled={verify_form.formState.isSubmitting}
                        >
                            Verify
                        </Button>
                    </div>
                </form>
            }/>
        </div>
    </div>;
}

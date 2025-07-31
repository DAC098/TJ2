import { useState } from "react";
import { useForm, SubmitHandler } from "react-hook-form";
import { useNavigate, useLocation, useSearchParams } from "react-router-dom";

import { ApiError, req_api_json, send_json } from "@/net";

import {
    InputOTP,
    InputOTPGroup,
    InputOTPSlot,
} from "@/components/ui/input-otp"
import { Button } from "@/components/ui/button";
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage, FormRootError } from "@/components/ui/form";
import { PasswordInput } from "@/components/ui/input";
import { useQueryClient } from "@tanstack/react-query";
import { curr_user_query_key } from "@/components/hooks/user";

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
    const [search_params, _] = useSearchParams();

    const client = useQueryClient();
    const [mfa_type, set_mfa_type] = useState<MFAType>(MFAType.Totp);

    const verify_form = useForm<VerifyForm>({
        defaultValues: {
            code: "",
        }
    });

    const on_submit: SubmitHandler<VerifyForm> = async (data, event) => {
        let prev = search_params.get("prev");

        try {
            verify_form.clearErrors();

            let res = await req_api_json("POST", "/verify", {type: get_mfa_type(mfa_type), ...data});

            client.setQueryData(curr_user_query_key(), {
                id: res.id,
                username: res.username,
            });

            navigate(prev ?? "/journals");
        } catch (err) {
            if (err instanceof ApiError) {
                switch (err.kind) {
                    case "InvalidCode":
                        verify_form.setError("code", {type: "custom", message: "Invalid TOTP code."});
                        break;
                    case "InvalidRecovery":
                        verify_form.setError("code", {type: "custom", message: "Invalid recovery code."});
                        break;
                    case "InvalidSession":
                        verify_form.setError("code", {type: "custom", message: "Invalid session. Return to login and try again."})
                        break;
                    case "AlreadyVerified":
                        navigate(prev ?? "/journals");
                        break;
                    case "MFANotFound":
                        verify_form.setError("code", {type: "custom", message: "MFA not enabled for this account."});
                        break;
                    default:
                        verify_form.setError("root", {message: "Failed to verify TOTP"});
                        break;
                }
            } else {
                console.error("failed to send verification", err);

                verify_form.setError("root", {message: "Error when sending verification. Try again."});
            }
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
                    <FormRootError/>
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

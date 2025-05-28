import { useState } from "react";
import { useForm, SubmitHandler } from "react-hook-form";
import { useNavigate, useLocation, Link } from "react-router-dom";

import { send_json } from "@/net";

import { Input, PasswordInput } from "@/components/ui/input";
import {
    InputOTP,
    InputOTPGroup,
    InputOTPSlot,
} from "@/components/ui/input-otp"
import { Button } from "@/components/ui/button";
import { Form, FormControl, FormDescription, FormField, FormItem, FormLabel, FormMessage } from "@/components/ui/form";

interface VerifyForm {
    code: string
}

export function Verify() {
    const navigate = useNavigate();
    const location = useLocation();

    const verify_form = useForm<VerifyForm>({
        defaultValues: {
            code: "",
        }
    });

    const on_submit: SubmitHandler<VerifyForm> = async (data, event) => {
        try {
            let res = await send_json("POST", "/verify", data);

            switch (res.status) {
                case 200:
                    let prev = new URL(location.pathname + location.search, window.location.origin)
                        .searchParams
                        .get("prev");

                    navigate(prev ?? "/journals");

                    break;
                case 400:
                    let json = await res.json();

                    if (json.type === "InvalidCode") {
                        console.log("invalid totp code");
                    } else if (json.type === "InvalidSession") {
                        navigate("/login");
                    } else if (json.type === "AlreadyVerified") {
                        navigate(prev ?? "/journals");
                    }

                    break;
                case 404:
                    let json = await res.json();

                    if (json.type === "MFANotFound") {
                        navigate("/login");
                    }

                    break;
                default:
                    console.warn("unhandled status code");

                    break;
            }
        } catch (err) {
            console.error("failed to send verification", err);
        }
    };

    return <div className="flex w-full h-full">
        <div className="mx-auto my-auto">
            <Form {...verify_form} children={
                <form className="space-y-4" onSubmit={verify_form.handleSubmit(on_submit)}>
                    <FormField control={verify_form.control} name="code" render={({field}) => {
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
                        </FormItem>;
                    }}/>
                    <div className="flex flex-row gap-x-4 justify-center">
                        <Button type="submit">Verify</Button>
                    </div>
                </form>
            }/>
        </div>
    </div>;
}

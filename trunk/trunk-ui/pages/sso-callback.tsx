import { AuthenticateWithRedirectCallback } from "@clerk/nextjs";
import { useUser, useSignIn, useClerk } from "@clerk/nextjs";

export default function SSOCallBack() {
  const { user } = useUser();

  console.log("SSO CALLBACK", user);

  return <AuthenticateWithRedirectCallback />;
}

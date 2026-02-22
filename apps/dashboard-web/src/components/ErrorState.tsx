import InlineBanner from "@/components/InlineBanner";

type Props = {
  message?: string;
};

const ErrorState = ({
  message = "Unable to load data from the core server.",
}: Props) => (
  <InlineBanner tone="danger" className="rounded-lg">
    {message}
  </InlineBanner>
);

export default ErrorState;

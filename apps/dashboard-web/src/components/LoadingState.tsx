import { Card } from "@/components/ui/card";

type Props = {
  label?: string;
};

const LoadingState = ({ label = "Loading data..." }: Props) => (
  <Card className="rounded-lg gap-0 border-dashed py-6 text-center text-sm text-muted-foreground">
    {label}
  </Card>
);

export default LoadingState;

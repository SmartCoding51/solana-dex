import React from "react";
import AddressView, { WrapperAddressViewProps } from "./AddressView";

const DestChatAddressView: React.FC<WrapperAddressViewProps> = ({
  address,
  setAddress,
}) => {
  return (
    <AddressView
      title="Dest. Chat Address"
      address={address}
      setAddress={setAddress}
      readOnly={false}
    />
  );
};

export default DestChatAddressView;

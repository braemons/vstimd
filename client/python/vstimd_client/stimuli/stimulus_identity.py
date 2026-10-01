from dataclasses import dataclass

from vstimd_client._proto.vstimd.v1.stimuli.identity_pb2 import (
    StimulusIdentity as ProtoStimulusIdentity,
)


@dataclass
class StimulusIdentity:
    name: str

    @classmethod
    def _from_proto(cls, proto: ProtoStimulusIdentity) -> 'StimulusIdentity':
        return cls(name=proto.name)

    def _to_proto(self) -> ProtoStimulusIdentity:
        return ProtoStimulusIdentity(name=self.name)

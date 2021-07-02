using System.Runtime.Serialization;

namespace Hoi4ModdingSupporter.Container
{
    [DataContract(Name = "TriggerContainer")]
    public class TriggerContainer
    {
        [DataMember(Name = "tag")]
        public string Tag { get; set; }
    }
}
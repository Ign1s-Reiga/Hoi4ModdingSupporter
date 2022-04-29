using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.Serialization;
using System.Text;
using System.Threading.Tasks;

namespace Hoi4ModdingSupporter.Container
{
    [DataContract(Name = "EventContainer")]
    class EventContainer
    {
        [DataMember(Name = "id")]
        public string Id { get; set; }

        [DataMember(Name = "title")]
        public string Title { get; set; }

        [DataMember(Name = "desc")]
        public string Description { get; set; }

        [DataMember(Name = "picture")]
        public FileStream PicturePath { get; set; }

        [DataMember(Name = "is_fired")]
        public bool IsFiredOnce { get; set; }

        [DataMember(Name = "is_triggerd")]
        public bool IsTriggeredOnly { get; set; }

        [DataMember(Name = "is_major")]
        public bool IsMajor { get; set; }

        [DataMember(Name = "is_hidden")]
        public bool IsHidden { get; set; }

        [DataMember(Name = "mtth")]
        public int MeanTimeToHappen { get; set; }

        [DataMember(Name = "timeout")]
        public int TimeOutDays { get; set; }

        [DataMember(Name = "trigger")]
        public TriggerContainer TriggerContainer { get; set; }

        [DataMember(Name = "options")]
        public List<OptionContainer> OptionContainerList { get; set; }
    }
}

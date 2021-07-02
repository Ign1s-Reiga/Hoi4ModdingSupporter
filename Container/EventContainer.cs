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
        public string ID { get; set; }

        [DataMember(Name = "title")]
        public string Title { get; set; }

        [DataMember(Name = "desc")]
        public string Description { get; set; }

        [DataMember(Name = "picture")]
        public FileStream PicturePath { get; set; }

        [DataMember(Name = "happen")]
        public bool IsHappenOnlyOnce { get; set; }

        [DataMember(Name = "trigger")]
        public TriggerContainer TriggerContainer { get; set; }
    }
}

using System;
using System.Collections.Generic;
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
        private string ID { get; set; }

        [DataMember(Name = "title")]
        private string Title { get; set; }

        [DataMember(Name = "desc")]
        private string Description { get; set; }
    }
}

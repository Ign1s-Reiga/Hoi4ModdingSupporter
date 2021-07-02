using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace Hoi4ModdingSupporter.Util
{
    class StringValueAttribute : Attribute
    {
        private string StringValue { get; set; }

        public StringValueAttribute(string value)
        {
            this.StringValue = value;
        }

        public string getValue()
        {
            return StringValue;
        }
    }
}
